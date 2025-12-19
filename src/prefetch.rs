//! Background prefetching of search results
//!
//! After search completes, this module downloads and processes all results
//! in parallel, saving them to current_search/. When user presses Enter,
//! the file is instantly moved to active_tabs/ and opened in Neovim.

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::extract_clean_md::extract_clean_markdown;
use crate::globals::get_http_client;
use crate::search::SearchResult;

/// Status of a prefetched page
#[derive(Debug, Clone, PartialEq)]
pub enum PrefetchStatus {
    /// Not yet started
    Pending,
    /// Currently downloading/processing
    InProgress,
    /// Successfully prefetched, file path stored
    Ready(PathBuf),
    /// Failed with error message
    Failed(String),
}

/// Manages prefetching of search results
pub struct PrefetchManager {
    /// Directory for current search results
    current_search_dir: PathBuf,
    /// Directory for active tabs (opened in neovim)
    active_tabs_dir: PathBuf,
    /// Status of each URL being prefetched
    status: Arc<RwLock<HashMap<String, PrefetchStatus>>>,
    /// Counter for completed prefetches
    completed_count: Arc<RwLock<usize>>,
    /// Total number of items to prefetch
    total_count: Arc<RwLock<usize>>,
}

impl PrefetchManager {
    /// Create a new prefetch manager
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        let current_search_dir = base_dir.join("current_search");
        let active_tabs_dir = base_dir.join("active_tabs");

        // Create directories
        std::fs::create_dir_all(&current_search_dir)
            .context("Failed to create current_search directory")?;
        std::fs::create_dir_all(&active_tabs_dir)
            .context("Failed to create active_tabs directory")?;

        Ok(Self {
            current_search_dir,
            active_tabs_dir,
            status: Arc::new(RwLock::new(HashMap::new())),
            completed_count: Arc::new(RwLock::new(0)),
            total_count: Arc::new(RwLock::new(0)),
        })
    }

    /// Clear previous search results and prepare for new search
    pub async fn clear_current_search(&self) -> Result<()> {
        // Clear status
        {
            let mut status = self.status.write().await;
            status.clear();
        }
        {
            let mut count = self.completed_count.write().await;
            *count = 0;
        }
        {
            let mut total = self.total_count.write().await;
            *total = 0;
        }

        // Remove old files from current_search
        if self.current_search_dir.exists() {
            let entries = std::fs::read_dir(&self.current_search_dir)?;
            for entry in entries.flatten() {
                if entry.path().extension().map_or(false, |e| e == "md") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }

        Ok(())
    }

    /// Start prefetching all search results in parallel
    ///
    /// This runs in the background and updates status as each page completes.
    pub async fn prefetch_all(&self, results: &[SearchResult]) {
        // Set total count
        {
            let mut total = self.total_count.write().await;
            *total = results.len();
        }

        // Mark all as pending
        {
            let mut status = self.status.write().await;
            for result in results {
                status.insert(result.url.clone(), PrefetchStatus::Pending);
            }
        }

        // Clone what we need for the async tasks
        let results: Vec<_> = results.to_vec();
        let status = Arc::clone(&self.status);
        let completed_count = Arc::clone(&self.completed_count);
        let current_search_dir = self.current_search_dir.clone();

        // Spawn prefetch tasks with concurrency limit
        tokio::spawn(async move {
            stream::iter(results.into_iter().enumerate())
                .for_each_concurrent(5, |(idx, result)| {
                    let status = Arc::clone(&status);
                    let completed_count = Arc::clone(&completed_count);
                    let dir = current_search_dir.clone();

                    async move {
                        // Mark as in progress
                        {
                            let mut s = status.write().await;
                            s.insert(result.url.clone(), PrefetchStatus::InProgress);
                        }

                        // Prefetch the page
                        let prefetch_result =
                            prefetch_single_page(&result, &dir, idx).await;

                        // Update status
                        {
                            let mut s = status.write().await;
                            match prefetch_result {
                                Ok(path) => {
                                    s.insert(result.url.clone(), PrefetchStatus::Ready(path));
                                }
                                Err(e) => {
                                    s.insert(
                                        result.url.clone(),
                                        PrefetchStatus::Failed(e.to_string()),
                                    );
                                }
                            }
                        }

                        // Increment completed count
                        {
                            let mut count = completed_count.write().await;
                            *count += 1;
                        }
                    }
                })
                .await;
        });
    }

    /// Get the prefetch status for a URL
    pub async fn get_status(&self, url: &str) -> PrefetchStatus {
        let status = self.status.read().await;
        status
            .get(url)
            .cloned()
            .unwrap_or(PrefetchStatus::Pending)
    }

    /// Get progress as (completed, total)
    pub async fn get_progress(&self) -> (usize, usize) {
        let completed = *self.completed_count.read().await;
        let total = *self.total_count.read().await;
        (completed, total)
    }

    /// Move a prefetched file from current_search to active_tabs
    ///
    /// Returns the final path in active_tabs/
    pub async fn activate_page(&self, url: &str) -> Result<PathBuf> {
        let status = self.get_status(url).await;

        match status {
            PrefetchStatus::Ready(source_path) => {
                let filename = source_path
                    .file_name()
                    .context("Invalid filename")?
                    .to_os_string();
                let dest_path = self.active_tabs_dir.join(filename);

                // Move file (or copy if on different filesystem)
                if let Err(_) = std::fs::rename(&source_path, &dest_path) {
                    // Fallback to copy + delete
                    std::fs::copy(&source_path, &dest_path)?;
                    let _ = std::fs::remove_file(&source_path);
                }

                Ok(dest_path)
            }
            PrefetchStatus::InProgress => {
                anyhow::bail!("Page is still loading...")
            }
            PrefetchStatus::Pending => {
                anyhow::bail!("Page prefetch not started")
            }
            PrefetchStatus::Failed(err) => {
                anyhow::bail!("Prefetch failed: {}", err)
            }
        }
    }
}

/// Prefetch a single page
async fn prefetch_single_page(
    result: &SearchResult,
    dir: &PathBuf,
    index: usize,
) -> Result<PathBuf> {
    let client = get_http_client();

    // Download HTML
    let response = client
        .get(&result.url)
        .header("Accept", "text/html,application/xhtml+xml")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .context("Failed to download page")?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {}", response.status());
    }

    let html = response
        .text()
        .await
        .context("Failed to read response body")?;

    // Extract content
    let content = extract_clean_markdown(&html, &result.url)
        .context("Failed to extract content")?;

    // Create filename with index for ordering
    let safe_title = sanitize_filename(&result.title);
    let filename = format!("{:02}_{}.md", index + 1, safe_title);
    let filepath = dir.join(&filename);

    // Save to file
    tokio::fs::write(&filepath, content.to_formatted_markdown())
        .await
        .context("Failed to save markdown file")?;

    Ok(filepath)
}

/// Create a safe filename from a title
fn sanitize_filename(title: &str) -> String {
    let safe: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Collapse multiple spaces/underscores
    let mut result = String::new();
    let mut prev_was_separator = false;
    for c in safe.chars() {
        if c == ' ' || c == '_' {
            if !prev_was_separator {
                result.push('_');
                prev_was_separator = true;
            }
        } else {
            result.push(c);
            prev_was_separator = false;
        }
    }

    // Truncate if too long
    if result.len() > 50 {
        result.truncate(50);
    }

    // Remove trailing underscore
    result.trim_end_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("Hello World! Test"),
            "Hello_World_Test"
        );
        assert_eq!(
            sanitize_filename("Multiple   Spaces"),
            "Multiple_Spaces"
        );
        assert_eq!(sanitize_filename("a".repeat(100).as_str()).len(), 50);
    }
}
