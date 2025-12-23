//! Terminal UI using ratatui

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;

use crate::app::{App, AppState};
use crate::prefetch::PrefetchStatus;

/// Draw the main UI
pub fn draw_ui(
    f: &mut Frame,
    app: &App,
    prefetch_progress: (usize, usize),
    statuses: &HashMap<String, PrefetchStatus>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Length(1), // Progress bar
            Constraint::Min(10),   // Results
            Constraint::Length(4), // Help bar (increased for status legend)
        ])
        .split(f.area());

    // Draw search input
    draw_search_input(f, app, chunks[0]);

    // Draw prefetch progress bar
    draw_progress_bar(f, prefetch_progress, chunks[1]);

    // Draw main content
    match app.state {
        AppState::Input | AppState::Results => {
            draw_results(f, app, chunks[2], statuses);
        }
        AppState::Searching => {
            draw_searching(f, chunks[2]);
        }
        AppState::Error => {
            draw_error(f, app, chunks[2]);
        }
    }

    // Draw help bar
    draw_help_bar(f, app, chunks[3]);
}

/// Draw search input field
fn draw_search_input(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.state == AppState::Input;

    let style = if is_focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let input = Paragraph::new(app.input.as_str()).style(style).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " 🔍 Search ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(if is_focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::Gray)
            }),
    );

    f.render_widget(input, area);

    if is_focused {
        f.set_cursor_position((
            area.x + app.cursor_pos as u16 + 1,
            area.y + 1
        ));
    }
}

/// Draw prefetch progress bar
fn draw_progress_bar(f: &mut Frame, progress: (usize, usize), area: Rect) {
    let (completed, total) = progress;

    if total == 0 {
        // No prefetching in progress, show empty line
        let empty = Paragraph::new("");
        f.render_widget(empty, area);
        return;
    }

    let ratio = if total > 0 {
        completed as f64 / total as f64
    } else {
        0.0
    };

    let color = if completed == total {
        Color::Green
    } else {
        Color::Yellow
    };

    let label = if completed == total {
        format!("✓ All {} pages ready", total)
    } else {
        format!("Prefetching: {}/{}", completed, total)
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color))
        .ratio(ratio)
        .label(Span::styled(label, Style::default().fg(Color::White)));

    f.render_widget(gauge, area);
}

/// Draw search results list with per-result status
fn draw_results(
    f: &mut Frame,
    app: &App,
    area: Rect,
    statuses: &HashMap<String, PrefetchStatus>,
) {
    if app.results.is_empty() {
        let message = if app.state == AppState::Input {
            "Enter your search query above and press Enter"
        } else {
            "No results found"
        };

        let paragraph = Paragraph::new(message)
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Results ")
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
        return;
    }

    let visible_height = area.height.saturating_sub(2) as usize;
    let scroll_offset = app.get_scroll_offset(visible_height);

    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height / 4 + 1)
        .map(|(i, result)| {
            let is_selected = i == app.selected_index;
            let is_marked = app.selected_items.contains(&i);

            // Get status for this result
            let status = statuses
                .get(&result.url)
                .cloned()
                .unwrap_or(PrefetchStatus::Pending);

            // Status icon and color
            let (status_icon, status_color) = match status {
                PrefetchStatus::Ready(_) => ("✓", Color::Green),
                PrefetchStatus::Cached(_) => ("📄", Color::Blue),
                PrefetchStatus::InProgress => ("⏳", Color::Yellow),
                PrefetchStatus::Failed(_) => ("⚠", Color::Red),
                PrefetchStatus::Timeout => ("⏱", Color::Red),
                PrefetchStatus::Pending => ("○", Color::DarkGray),
            };

            // Selection indicator
            let select_char = if is_marked { "✓" } else { " " };
            let number = format!("{:2}.", i + 1);

            let content = vec![
                Line::from(vec![
                    Span::styled(
                        select_char,
                        Style::default().fg(if is_marked {
                            Color::Green
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::styled(number, Style::default().fg(Color::Yellow)),
                    Span::raw(" "),
                    Span::styled(status_icon, Style::default().fg(status_color)),
                    Span::raw(" "),
                    Span::styled(
                        &result.title,
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(truncate(&result.url, 80), Style::default().fg(Color::Blue)),
                ]),
                Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        truncate(&result.description, 100),
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::raw(""),
            ];

            let style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(35, 35, 45))  // Dark blue
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let title = format!(" 📊 Results ({}) ", app.results.len());

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, area);
}

/// Draw searching indicator
fn draw_searching(f: &mut Frame, area: Rect) {
    let paragraph = Paragraph::new("⏳ Searching...")
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Status ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Draw error message
fn draw_error(f: &mut Frame, app: &App, area: Rect) {
    let error_text = app.error_message.as_deref().unwrap_or("Unknown error");

    let paragraph = Paragraph::new(format!(
        "❌ Error: {}\n\nPress any key to continue...",
        error_text
    ))
    .style(Style::default().fg(Color::Red))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                " Error ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(Color::Red)),
    )
    .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Draw help bar with status legend
fn draw_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.state {
        AppState::Input => "Enter: Brave │ Ctrl+D: DuckDuckGo │ Ctrl+X: SearXNG │ Ctrl+Z: Startpage │ Esc: Clear │ Ctrl+Q: Quit",
        AppState::Results => {
            "↑/k ↓/j: Navigate │ gg/G: First/Last │ Tab: Select │ Enter: Neovim │ Ctrl+B: Browser │ Esc: New Search │ Ctrl+Q: Quit\nStatus: ✓=Ready 📄=Cached ⏳=Loading ⚠=Failed ⏱=Timeout"
        }
        AppState::Searching => "⏳ Please wait... │ Ctrl+Q: Quit",
        AppState::Error => "Press any key to continue │ Ctrl+Q: Quit",
    };

    let paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Truncate string to max length
fn truncate(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();

    if char_count <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}