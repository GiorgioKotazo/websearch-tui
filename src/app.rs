//! Application state and core logic

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;

use crate::prefetch::{PrefetchManager, PrefetchStatus};
use crate::search::SearchResult;

/// Messages sent from background tasks to the main app
#[derive(Debug)]
pub enum AppMessage {
    /// Search completed with results
    SearchComplete(Vec<SearchResult>),
    /// Search failed with error
    SearchError(String),
}

/// Application state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// User is typing search query
    Input,
    /// Performing search
    Searching,
    /// Showing search results (prefetching in background)
    Results,
    /// Showing error message
    Error,
}

/// Main application structure
pub struct App {
    pub state: AppState,
    pub input: String,
    pub results: Vec<SearchResult>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub selected_items: HashSet<usize>,
    pub error_message: Option<String>,
    pub prefetch_manager: PrefetchManager,
    /// Status message shown in UI
    pub status_message: String,
}

impl App {
    /// Create new app instance
    pub fn new() -> Result<Self> {
        let base_dir = PathBuf::from("websearch");
        let prefetch_manager = PrefetchManager::new(base_dir)?;

        // Spawn background cleanup task (removes files older than 5 days)
        let pm_clone = prefetch_manager.clone();
        tokio::spawn(async move {
            if let Ok(count) = pm_clone.cleanup_old_files().await {
                if count > 0 {
                    eprintln!("🧹 Cleaned up {} old cache files", count);
                }
            }
        });

        Ok(Self {
            state: AppState::Input,
            input: String::new(),
            results: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            selected_items: HashSet::new(),
            error_message: None,
            prefetch_manager,
            status_message: String::new(),
        })
    }

    /// Start search operation
    pub async fn start_search(&mut self) {
        self.state = AppState::Searching;
        self.results.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.selected_items.clear();
        self.status_message = "Searching...".to_string();

        // Clear previous search cache
        if let Err(e) = self.prefetch_manager.clear_current_search().await {
            self.status_message = format!("Warning: {}", e);
        }
    }

    /// Finish search with results and start prefetching
    pub async fn finish_search(&mut self, results: Vec<SearchResult>) {
        if results.is_empty() {
            self.error_message = Some("No results found".to_string());
            self.state = AppState::Error;
            return;
        }

        let count = results.len();
        self.results = results;
        self.state = AppState::Results;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.status_message = format!("Found {} results. Prefetching...", count);

        // Start prefetching all results in background (with caching)
        self.prefetch_manager.prefetch_all(&self.results).await;
    }

    /// Update prefetch progress
    pub fn update_prefetch_progress(&mut self, completed: usize, total: usize) {
        if completed == total {
            self.status_message = format!("✓ All {} pages ready!", total);
        } else {
            self.status_message = format!("Prefetching: {}/{}", completed, total);
        }
    }

    /// Show error message
    pub fn show_error(&mut self, message: &str) {
        self.error_message = Some(message.to_string());
        self.state = AppState::Error;
    }

    /// Dismiss error
    pub fn dismiss_error(&mut self) {
        self.error_message = None;
        self.state = if self.results.is_empty() {
            AppState::Input
        } else {
            AppState::Results
        };
    }

    /// Move to next result
    pub fn next_result(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }

    /// Move to previous result
    pub fn previous_result(&mut self) {
        if !self.results.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.results.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Jump to first result
    pub fn first_result(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Jump to last result
    pub fn last_result(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = self.results.len() - 1;
        }
    }

    /// Get scroll offset for rendering
    pub fn get_scroll_offset(&self, visible_height: usize) -> usize {
        let items_per_screen = visible_height.saturating_sub(2) / 4;

        if self.selected_index >= items_per_screen {
            self.selected_index.saturating_sub(items_per_screen - 1)
        } else {
            0
        }
    }

    /// Toggle selection of current item
    pub fn toggle_selection(&mut self) {
        if self.selected_items.contains(&self.selected_index) {
            self.selected_items.remove(&self.selected_index);
        } else {
            self.selected_items.insert(self.selected_index);
        }
    }

    /// Open selected items in browser
    pub fn open_in_browser(&mut self) {
        let indices: Vec<usize> = if self.selected_items.is_empty() {
            vec![self.selected_index]
        } else {
            self.selected_items.iter().copied().collect()
        };

        for &idx in &indices {
            if let Some(result) = self.results.get(idx) {
                if let Err(e) = open_url(&result.url) {
                    self.show_error(&format!("Failed to open URL: {}", e));
                    return;
                }
            }
        }

        self.selected_items.clear();
        self.status_message = format!("Opened {} URL(s) in browser", indices.len());
    }

    /// Open current result in neovim
    ///
    /// This activates the page (moves from current_search to active_tabs)
    /// and returns the filepath to open.
    pub async fn prepare_neovim_open(&mut self) -> Result<PathBuf> {
        let result = self
            .results
            .get(self.selected_index)
            .context("No result selected")?;

        // Activate the page (move to active_tabs)
        let filepath = self
            .prefetch_manager
            .activate_page(&result.url)
            .await
            .context("Failed to activate page")?;

        Ok(filepath)
    }

    /// Go back to input mode
    pub fn back_to_input(&mut self) {
        self.state = AppState::Input;
    }

    /// Get prefetch progress
    pub async fn get_prefetch_progress(&self) -> (usize, usize) {
        self.prefetch_manager.get_progress().await
    }

    /// Get prefetch status for a specific URL
    pub async fn get_prefetch_status(&self, url: &str) -> PrefetchStatus {
        self.prefetch_manager.get_status(url).await
    }

    /// Get all prefetch statuses (for UI rendering)
    pub async fn get_all_statuses(&self) -> HashMap<String, PrefetchStatus> {
        self.prefetch_manager.get_all_statuses().await
    }
}

/// Open URL in default browser
fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to open browser")?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to open browser")?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/C", "start", "", url])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to open browser")?;
    }

    Ok(())
}

/// Open file in neovim (blocking)
pub fn open_in_neovim(filepath: &PathBuf) -> Result<()> {
    let status = Command::new("nvim")
        .arg(filepath)
        .status()
        .context("Failed to launch neovim")?;

    if !status.success() {
        anyhow::bail!("Neovim exited with error");
    }

    Ok(())
}