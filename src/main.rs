mod search;
mod extract_clean_md;
mod app;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use dotenvy::dotenv;
use tokio::sync::mpsc;

use app::{App, AppState, AppMessage};
use ui::draw_ui;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file
    dotenv().ok();

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
    loop {
        // Check for messages from background tasks (non-blocking)
        while let Ok(msg) = rx.try_recv() {
            match msg {
                AppMessage::SearchComplete(results) => {
                    app.finish_search(results);
                }
                AppMessage::SearchError(err) => {
                    app.show_error(&format!("Search failed: {}", err));
                }
            }
        }

        // Draw UI
        terminal.draw(|f| draw_ui(f, app))?;

        // Handle input with timeout - only read ONE event per loop iteration
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events, ignore release and repeat
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                
                match app.state {
                    AppState::Input => {
                        match key.code {
                            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(());
                            }
                            KeyCode::Char(c) => {
                                app.input.push(c);
                            }
                            KeyCode::Backspace => {
                                app.input.pop();
                            }
                            KeyCode::Enter => {
                                if !app.input.trim().is_empty() {
                                    // Start search
                                    let query = app.input.clone();
                                    app.start_search();
                                    
                                    // Spawn background task for search
                                    let tx_clone = tx.clone();
                                    tokio::spawn(async move {
                                        let api_key = std::env::var("BRAVE_SEARCH_API_KEY")
                                            .unwrap_or_else(|_| String::new());
                                        
                                        if api_key.is_empty() {
                                            let _ = tx_clone.send(AppMessage::SearchError(
                                                "BRAVE_SEARCH_API_KEY not set in environment".to_string()
                                            ));
                                        } else {
                                            match search::brave_search(&api_key, &query).await {
                                                Ok(results) => {
                                                    let _ = tx_clone.send(AppMessage::SearchComplete(results));
                                                }
                                                Err(e) => {
                                                    let _ = tx_clone.send(AppMessage::SearchError(e.to_string()));
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                            KeyCode::Esc => {
                                app.input.clear();
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
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.previous_result();
                            }
                            KeyCode::Tab => {
                                app.toggle_selection();
                            }
                            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                // Open selected results in browser
                                app.open_in_browser();
                            }
                            KeyCode::Enter => {
                                // Open in neovim (need to exit TUI temporarily)
                                if let Some(nvim_request) = app.prepare_neovim_open() {
                                    // Exit TUI mode
                                    disable_raw_mode()?;
                                    execute!(
                                        io::stdout(),
                                        LeaveAlternateScreen,
                                        DisableMouseCapture
                                    )?;
                                    
                                    // Open in neovim (blocking)
                                    app.state = AppState::Searching;
                                    let result = crate::app::open_in_neovim_blocking(&nvim_request).await;
                                    
                                    // Re-enter TUI mode
                                    enable_raw_mode()?;
                                    execute!(
                                        io::stdout(),
                                        EnterAlternateScreen,
                                        EnableMouseCapture
                                    )?;
                                    terminal.clear()?;
                                    
                                    // Handle result
                                    match result {
                                        Ok(_) => {
                                            app.state = AppState::Results;
                                        }
                                        Err(e) => {
                                            app.show_error(&format!("Failed to open in neovim: {}", e));
                                        }
                                    }
                                }
                            }
                            KeyCode::Esc => {
                                // Go back to search input
                                app.back_to_input();
                            }
                            _ => {}
                        }
                    }
                    AppState::Searching => {
                        // Can't do anything while searching, just wait
                        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
                            return Ok(());
                        }
                    }
                    AppState::Error => {
                        // Any key dismisses the error
                        app.dismiss_error();
                    }
                }
            }
        }
    }
}