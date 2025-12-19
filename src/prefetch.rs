//! Background prefetching of search results
//!
//! After search completes, this module downloads and processes results
//! in parallel (12 concurrent), with intelligent caching and 8-second timeouts.

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::timeout;
use url::Url;

use crate::extract_clean_md::extract_clean_markdown;
use crate::globals::get_http_client;
use crate::search::SearchResult;

/// Concurrency limit for parallel downloads
const CONCURRENT_LIMIT: usize = 12;

/// Per-page timeout (fail fast on slow sites)
const PAGE_TIMEOUT: Duration = Duration::from_secs(8);

/// Maximum cache age in days
const CACHE_MAX_AGE_DAYS: u64 = 5;

/// Status of a prefetched page
#[derive(Debug, Clone, PartialEq)]
pub enum PrefetchStatus {
    /// Not yet started
    Pending,
    /// Currently downloading/processing
    InProgress,
    /// Successfully prefetched, file path stored
    Ready(PathBuf),
    /// Already existed on disk (cached)
    Cached(PathBuf),
    /// Failed with error message
    Failed(String),
    /// Timed out after 8 seconds
    Timeout,
}

/// Manages prefetching of search results
#[derive(Clone)]
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

    /// Start prefetching search results with intelligent caching
    ///
    /// Checks if files already exist before downloading.
    /// Runs with 12 concurrent tasks and 8-second per-page timeout.
    pub async fn prefetch_all(&self, results: &[SearchResult]) {
        // Set total count
        {
            let mut total = self.total_count.write().await;
            *total = results.len();
        }

        // Check which files already exist (caching)
        let mut to_fetch = Vec::new();
        let mut cached = Vec::new();

        for result in results {
            let filename = url_to_filename(&result.url, &result.title);

            // Check active_tabs first
            let active_path = self.active_tabs_dir.join(&filename);
            if active_path.exists() {
                cached.push((result.clone(), active_path));
                continue;
            }

            // Check current_search
            let current_path = self.current_search_dir.join(&filename);
            if current_path.exists() {
                cached.push((result.clone(), current_path));
                continue;
            }

            // Need to fetch
            to_fetch.push(result.clone());
        }

        // Mark cached items as Cached immediately
        {
            let mut status = self.status.write().await;
            for (result, path) in cached {
                status.insert(result.url.clone(), PrefetchStatus::Cached(path));
            }
        }

        // Update completed count (cached items are already "done")
        {
            let mut completed = self.completed_count.write().await;
            *completed = results.len() - to_fetch.len();
        }

        // Mark items to fetch as Pending
        {
            let mut status = self.status.write().await;
            for result in &to_fetch {
                status.insert(result.url.clone(), PrefetchStatus::Pending);
            }
        }

        // Clone what we need for the async tasks
        let status = Arc::clone(&self.status);
        let completed_count = Arc::clone(&self.completed_count);
        let current_search_dir = self.current_search_dir.clone();

        // Spawn prefetch tasks with concurrency limit and timeout
        tokio::spawn(async move {
            stream::iter(to_fetch.into_iter())
                .for_each_concurrent(CONCURRENT_LIMIT, |result| {
                    let status = Arc::clone(&status);
                    let completed_count = Arc::clone(&completed_count);
                    let dir = current_search_dir.clone();

                    async move {
                        // Mark as in progress
                        {
                            let mut s = status.write().await;
                            s.insert(result.url.clone(), PrefetchStatus::InProgress);
                        }

                        // Wrap in timeout
                        let fetch_result = timeout(
                            PAGE_TIMEOUT,
                            prefetch_single_page(&result, &dir)
                        ).await;

                        // Update status
                        {
                            let mut s = status.write().await;
                            match fetch_result {
                                Ok(Ok(path)) => {
                                    s.insert(result.url.clone(), PrefetchStatus::Ready(path));
                                }
                                Ok(Err(e)) => {
                                    s.insert(
                                        result.url.clone(),
                                        PrefetchStatus::Failed(e.to_string()),
                                    );
                                }
                                Err(_) => {
                                    s.insert(result.url.clone(), PrefetchStatus::Timeout);
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

    /// Get all current statuses (for UI rendering)
    pub async fn get_all_statuses(&self) -> HashMap<String, PrefetchStatus> {
        let status = self.status.read().await;
        status.clone()
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
            PrefetchStatus::Ready(source_path) | PrefetchStatus::Cached(source_path) => {
                let filename = source_path
                    .file_name()
                    .context("Invalid filename")?;
                let dest_path = self.active_tabs_dir.join(filename);

                // If already in active_tabs, just return path
                if source_path.starts_with(&self.active_tabs_dir) {
                    return Ok(source_path);
                }

                // Move file (or copy if on different filesystem)
                if std::fs::rename(&source_path, &dest_path).is_err() {
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
            PrefetchStatus::Timeout => {
                anyhow::bail!("Page timed out after 8 seconds")
            }
        }
    }

    /// Clean up files older than CACHE_MAX_AGE_DAYS
    pub async fn cleanup_old_files(&self) -> Result<usize> {
        let max_age = Duration::from_secs(CACHE_MAX_AGE_DAYS * 24 * 60 * 60);
        let now = SystemTime::now();
        let mut removed_count = 0;

        // Clean active_tabs
        removed_count += self
            .cleanup_directory(&self.active_tabs_dir, now, max_age)
            .await?;

        // Clean current_search
        removed_count += self
            .cleanup_directory(&self.current_search_dir, now, max_age)
            .await?;

        Ok(removed_count)
    }

    async fn cleanup_directory(
        &self,
        dir: &PathBuf,
        now: SystemTime,
        max_age: Duration,
    ) -> Result<usize> {
        let mut removed = 0;

        if !dir.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(dir)?;
        for entry in entries.flatten() {
            if !entry.path().extension().map_or(false, |e| e == "md") {
                continue;
            }

            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            if std::fs::remove_file(entry.path()).is_ok() {
                                removed += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(removed)
    }
}

/// Prefetch a single page
async fn prefetch_single_page(result: &SearchResult, dir: &PathBuf) -> Result<PathBuf> {
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

    // Extract content (now using dom_smoothie)
    let content = extract_clean_markdown(&html, &result.url)
        .context("Failed to extract content")?;

    // Generate filename using new format: {domain}_{hash}_{title}.md
    let filename = url_to_filename(&result.url, &result.title);
    let filepath = dir.join(&filename);

    // Save to file
    tokio::fs::write(&filepath, content.to_formatted_markdown())
        .await
        .context("Failed to save markdown file")?;

    Ok(filepath)
}

/// Generate deterministic filename from URL
///
/// Format: {domain}_{hash_short}_{title}.md
/// Example: github_com_a3f8d912_Rust_Programming_Guide.md
fn url_to_filename(url: &str, title: &str) -> String {
    // Extract domain
    let domain = Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| "unknown".to_string());

    // Clean domain (remove www., replace dots with underscores)
    let clean_domain = domain
        .trim_start_matches("www.")
        .replace('.', "_");

    // Generate short hash (8 hex chars)
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = format!("{:08x}", hasher.finish() & 0xFFFFFFFF);

    // Clean title (max 30 chars)
    let safe_title = sanitize_filename(title);
    let truncated: String = safe_title.chars().take(30).collect();

    format!("{}_{}_{}.md", clean_domain, hash, truncated)
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
        assert_eq!(sanitize_filename("Multiple   Spaces"), "Multiple_Spaces");
    }

    #[test]
    fn test_url_to_filename() {
        let filename = url_to_filename(
            "https://github.com/rust-lang/rust",
            "The Rust Programming Language"
        );
        assert!(filename.starts_with("github_com_"));
        assert!(filename.contains("The_Rust_Programming"));
        assert!(filename.ends_with(".md"));
    }
}