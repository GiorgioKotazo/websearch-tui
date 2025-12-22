//! websearch-tui - Lightning-fast terminal web search with Neovim integration
//!
//! This TUI application provides:
//! - Fast web search via Brave Search API or DuckDuckGo
//! - Background prefetching with intelligent caching (12 concurrent, 8s timeout)
//! - Clean markdown extraction from web pages (dom_smoothie)
//! - Seamless Neovim integration for reading
//! - Auto-cleanup of files older than 5 days

mod app;
mod duckduckgo_search;
mod extract_clean_md;
mod globals;
mod prefetch;
mod search;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dotenvy::dotenv;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

use app::{App, AppMessage, AppState};
use ui::draw_ui;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize global resources (HTTP client)
    // This happens once at startup, avoiding delays during use
    globals::init_globals()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new()?;

    // Create channel for background tasks
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Run the app
    let res = run_app(&mut terminal, &mut app, tx, &mut rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    tx: mpsc::UnboundedSender<AppMessage>,
    rx: &mut mpsc::UnboundedReceiver<AppMessage>,
) -> Result<()> {
    // Track 'g' key for gg command
    let mut last_g_press: Option<std::time::Instant> = None;

    loop {
        // Check for messages from background tasks
        while let Ok(msg) = rx.try_recv() {
            match msg {
                AppMessage::SearchComplete(results) => {
                    app.finish_search(results).await;
                }
                AppMessage::SearchError(err) => {
                    app.show_error(&format!("Search failed: {}", err));
                }
            }
        }

        // Get prefetch progress and all statuses for UI
        let prefetch_progress = app.get_prefetch_progress().await;
        let statuses = app.get_all_statuses().await;

        // Update progress in status
        if app.state == AppState::Results {
            let (completed, total) = prefetch_progress;
            if total > 0 && completed < total {
                app.update_prefetch_progress(completed, total);
            }
        }

        // Draw UI
        terminal.draw(|f| draw_ui(f, app, prefetch_progress, &statuses))?;

        // Handle input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.state {
                    AppState::Input => {
                        match key.code {
                            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(());
                            }
                            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                // Ctrl+D: DuckDuckGo search
                                if !app.input.trim().is_empty() {
                                    let query = app.input.clone();
                                    app.start_search().await;

                                    // Spawn DuckDuckGo search task
                                    let tx_clone = tx.clone();
                                    tokio::spawn(async move {
                                        match duckduckgo_search::duckduckgo_search(&query).await {
                                            Ok(results) => {
                                                let _ = tx_clone
                                                    .send(AppMessage::SearchComplete(results));
                                            }
                                            Err(e) => {
                                                let _ = tx_clone.send(AppMessage::SearchError(
                                                    e.to_string(),
                                                ));
                                            }
                                        }
                                    });
                                }
                            }
                            KeyCode::Char(c) => {
                                app.insert_char(c);
                            }
                            KeyCode::Backspace => {
                                app.delete_char_before();
                            }
                            KeyCode::Delete => {
                                app.delete_char_after();
                            }
                            KeyCode::Left => {
                                app.cursor_left();
                            }
                            KeyCode::Right => {
                                app.cursor_right();
                            }
                            KeyCode::Home => {
                                app.cursor_home();
                            }
                            KeyCode::End => {
                                app.cursor_end();
                            }
                            KeyCode::Enter => {
                                if !app.input.trim().is_empty() {
                                    let query = app.input.clone();
                                    app.start_search().await;

                                    // Spawn search task
                                    let tx_clone = tx.clone();
                                    tokio::spawn(async move {
                                        // Use Brave search (Enter)
                                        let api_key = std::env::var("BRAVE_SEARCH_API_KEY")
                                            .unwrap_or_default();

                                        if api_key.is_empty() {
                                            let _ = tx_clone.send(AppMessage::SearchError(
                                                "BRAVE_SEARCH_API_KEY not set".to_string(),
                                            ));
                                        } else {
                                            match search::brave_search(&api_key, &query).await {
                                                Ok(results) => {
                                                    let _ = tx_clone
                                                        .send(AppMessage::SearchComplete(results));
                                                }
                                                Err(e) => {
                                                    let _ = tx_clone.send(AppMessage::SearchError(
                                                        e.to_string(),
                                                    ));
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                            KeyCode::Esc => {
                                app.clear_input();
                            }
                            _ => {}
                        }
                    }
                    AppState::Results => {
                        match key.code {
                            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(());
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next_result();
                                last_g_press = None;
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.previous_result();
                                last_g_press = None;
                            }
                            KeyCode::Char('g') => {
                                // Check for gg (go to top)
                                if let Some(last) = last_g_press {
                                    if last.elapsed() < Duration::from_millis(500) {
                                        app.first_result();
                                        last_g_press = None;
                                    } else {
                                        last_g_press = Some(std::time::Instant::now());
                                    }
                                } else {
                                    last_g_press = Some(std::time::Instant::now());
                                }
                            }
                            KeyCode::Char('G') => {
                                // Go to bottom
                                app.last_result();
                                last_g_press = None;
                            }
                            KeyCode::Tab => {
                                app.toggle_selection();
                                last_g_press = None;
                            }
                            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.open_in_browser();
                                last_g_press = None;
                            }
                            KeyCode::Enter => {
                                last_g_press = None;

                                // Check if multiple items selected - prevent multi-neovim
                                if !app.selected_items.is_empty() {
                                    app.status_message = "⚠ Can't open multiple files in Neovim. Unselect with Tab, or use Ctrl+B for browser".to_string();
                                    continue;
                                }

                                // Try to open in neovim
                                match app.prepare_neovim_open().await {
                                    Ok(filepath) => {
                                        // Exit TUI mode
                                        disable_raw_mode()?;
                                        execute!(
                                            io::stdout(),
                                            LeaveAlternateScreen,
                                            DisableMouseCapture
                                        )?;

                                        // Open in neovim (blocking)
                                        let result = app::open_in_neovim(&filepath);

                                        // Re-enter TUI mode
                                        enable_raw_mode()?;
                                        execute!(
                                            io::stdout(),
                                            EnterAlternateScreen,
                                            EnableMouseCapture
                                        )?;
                                        terminal.clear()?;

                                        if let Err(e) = result {
                                            app.show_error(&format!("Neovim error: {}", e));
                                        }
                                    }
                                    Err(e) => {
                                        app.status_message = format!("⏳ {}", e);
                                    }
                                }
                            }
                            KeyCode::Esc => {
                                app.back_to_input();
                                last_g_press = None;
                            }
                            _ => {
                                last_g_press = None;
                            }
                        }
                    }
                    AppState::Searching => {
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('q')
                        {
                            return Ok(());
                        }
                    }
                    AppState::Error => {
                        app.dismiss_error();
                    }
                }
            }
        }
    }
}