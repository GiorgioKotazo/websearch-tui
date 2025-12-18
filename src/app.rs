use anyhow::{Result, Context};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::search::SearchResult;
use crate::extract_clean_md::extract_clean_markdown;

/// Messages sent from background tasks to the main app
#[derive(Debug)]
pub enum AppMessage {
    SearchComplete(Vec<SearchResult>),
    SearchError(String),
}

/// Request to open a page in neovim
#[derive(Debug, Clone)]
pub struct NvimRequest {
    pub url: String,
    pub _title: String,  // Kept for potential future use
    pub filepath: PathBuf,
}

/// Application state
#[derive(Debug, PartialEq)]
pub enum AppState {
    Input,      // User is typing search query
    Searching,  // Performing search or loading page
    Results,    // Showing search results
    Error,      // Showing error message
}

/// Main application structure
pub struct App {
    pub state: AppState,
    pub input: String,
    pub results: Vec<SearchResult>,
    pub selected_index: usize,
    pub scroll_offset: usize,  // For scrolling large result lists
    pub selected_items: HashSet<usize>, // Indices of items selected with Tab
    pub error_message: Option<String>,
    pub active_tabs_dir: PathBuf,
}

impl App {
    /// Create new app instance and ensure necessary directories exist
    pub fn new() -> Result<Self> {
        let active_tabs_dir = PathBuf::from("websearch/active_tabs");
        
        // Create directories if they don't exist
        fs::create_dir_all(&active_tabs_dir)
            .context("Failed to create websearch/active_tabs directory")?;

        Ok(Self {
            state: AppState::Input,
            input: String::new(),
            results: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            selected_items: HashSet::new(),
            error_message: None,
            active_tabs_dir,
        })
    }

    /// Start search operation
    pub fn start_search(&mut self) {
        self.state = AppState::Searching;
        self.results.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.selected_items.clear();
    }

    /// Finish search with results
    pub fn finish_search(&mut self, results: Vec<SearchResult>) {
        self.results = results;
        self.state = if self.results.is_empty() {
            self.error_message = Some("No results found".to_string());
            AppState::Error
        } else {
            AppState::Results
        };
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Show error message
    pub fn show_error(&mut self, message: &str) {
        self.error_message = Some(message.to_string());
        self.state = AppState::Error;
    }

    /// Dismiss error and go back to appropriate state
    pub fn dismiss_error(&mut self) {
        self.error_message = None;
        if self.results.is_empty() {
            self.state = AppState::Input;
        } else {
            self.state = AppState::Results;
        }
    }

    /// Move to next result with proper scrolling
    pub fn next_result(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }

    /// Move to previous result with proper scrolling
    pub fn previous_result(&mut self) {
        if !self.results.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.results.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Get scroll offset for rendering
    pub fn get_scroll_offset(&self, visible_height: usize) -> usize {
        // Calculate how many items fit on screen (accounting for 4 lines per item + borders)
        let items_per_screen = visible_height.saturating_sub(2) / 4; // 4 lines per item
        
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

    /// Open selected or multi-selected items in browser
    pub fn open_in_browser(&mut self) {
        let indices_to_open: Vec<usize> = if self.selected_items.is_empty() {
            // No multi-selection, open current item
            vec![self.selected_index]
        } else {
            // Open all selected items
            self.selected_items.iter().copied().collect()
        };

        for &idx in &indices_to_open {
            if let Some(result) = self.results.get(idx) {
                if let Err(e) = open_url(&result.url) {
                    self.show_error(&format!("Failed to open URL: {}", e));
                    return;
                }
            }
        }

        // Clear multi-selection after opening
        self.selected_items.clear();
    }

    /// Prepare data for opening in neovim (returns data needed for opening)
    pub fn prepare_neovim_open(&self) -> Option<NvimRequest> {
        if self.selected_index >= self.results.len() {
            return None;
        }

        let result = &self.results[self.selected_index];
        let filename = create_safe_filename(&result.url);
        let filepath = self.active_tabs_dir.join(&filename);

        Some(NvimRequest {
            url: result.url.clone(),
            _title: result.title.clone(),
            filepath,
        })
    }

    /// Go back to search input, keeping results
    pub fn back_to_input(&mut self) {
        self.state = AppState::Input;
        // Note: We don't clear results here, they stay visible in the UI
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

/// Create safe filename from URL
fn create_safe_filename(url: &str) -> String {
    // Use URL as base, but make it filesystem-safe
    let safe_name: String = url
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    
    // Truncate if too long and add .md extension
    let max_len = 200;
    if safe_name.len() > max_len {
        format!("{}.md", &safe_name[..max_len])
    } else {
        format!("{}.md", safe_name)
    }
}

/// Open page in neovim (blocking operation, terminal modes already handled by caller)
pub async fn open_in_neovim_blocking(request: &NvimRequest) -> Result<()> {
    // Check if file already exists
    if !request.filepath.exists() {
        // Need to download and extract
        
        // Download HTML
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()?;
        
        let html = client
            .get(&request.url)
            .send()
            .await
            .context("Failed to download page")?
            .text()
            .await
            .context("Failed to read page content")?;

        // Extract and convert to markdown
        let extracted_content = extract_clean_markdown(&html, &request.url)
            .context("Failed to extract content from page")?;
        
        // Save to file
        fs::write(&request.filepath, extracted_content.to_formatted_markdown())
            .context("Failed to save markdown file")?;
    }

    // Open in neovim (blocking - terminal is already in normal mode)
    let status = Command::new("nvim")
        .arg(&request.filepath)
        .status()
        .context("Failed to launch neovim")?;
    
    if !status.success() {
        anyhow::bail!("Neovim exited with error");
    }
    
    Ok(())
}