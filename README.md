# WebSearch TUI

A beautiful terminal user interface (TUI) for web search with Brave Search API, featuring intelligent content extraction and Neovim integration.

## Features

- 🎨 **Beautiful TUI Interface** - Built with ratatui for a modern terminal experience
- 🔍 **Brave Search Integration** - Fast and private web search
- 📝 **Smart Content Extraction** - Automatically cleans and converts web pages to readable Markdown
- 💾 **Local Caching** - Downloaded pages are cached for instant re-access
- 🎯 **Multi-Selection** - Select multiple results to open at once
- ⌨️ **Vim Keybindings** - Navigate with j/k or arrow keys
- 📖 **Neovim Integration** - Read articles directly in your editor

## Prerequisites

- Rust (latest stable version)
- Neovim installed and in PATH
- Brave Search API key (get one at https://brave.com/search/api/)

## Installation

1. Clone the repository
2. Create a `.env` file in the project root:
   ```
   BRAVE_SEARCH_API_KEY=your_api_key_here
   ```
3. Build and run:
   ```bash
   cargo build --release
   cargo run
   ```

## Usage

### Search Interface

1. **Launch the app** - The search input field is focused by default
2. **Type your query** and press `Enter` to search
3. **Navigate results**:
   - Use `↑`/`↓` or `k`/`j` to move between results
   - Press `Tab` to select/deselect multiple results (✓ marker appears)
   - Press `Enter` to open current result in Neovim
   - Press `Ctrl+B` to open selected result(s) in browser
   - Press `Esc` to start a new search
   - Press `Ctrl+Q` to quit the application

### Opening in Neovim

When you press `Enter` on a result:
- The app checks if the page is already cached in `websearch/active_tabs/`
- If cached, it opens immediately in Neovim
- If not, it downloads the page, extracts clean content, converts to Markdown, caches it, then opens in Neovim
- After closing Neovim, you return to the search results

### Opening in Browser

When you press `Ctrl+B`:
- If you have selected multiple results with `Tab`, all selected results open
- If no multi-selection, the current result opens
- The app remains open with results visible

### File Organization

All extracted web pages are saved as Markdown files in:
```
websearch/active_tabs/
```

Each file is named based on the URL for easy identification and reuse.

## Keyboard Shortcuts

### Search Input Mode
- `Enter` - Perform search
- `Esc` - Clear input
- `Ctrl+Q` - Quit

### Results Mode
- `↑` or `k` - Move up
- `↓` or `j` - Move down
- `Tab` - Toggle selection of current result
- `Enter` - Open in Neovim
- `Ctrl+B` - Open in browser
- `Esc` - New search
- `Ctrl+Q` - Quit

## Error Handling

If an error occurs:
- The error message is displayed in a red panel
- Press any key to dismiss and continue
- Common errors include:
  - Missing API key
  - Network failures
  - Page extraction failures
  - Neovim not found in PATH

## Directory Structure

```
websearch-tui/
├── src/
│   ├── main.rs              # Entry point and event loop
│   ├── app.rs               # Application state management
│   ├── ui.rs                # TUI rendering logic
│   ├── search.rs            # Brave Search API integration
│   └── extract_clean_md.rs  # HTML to Markdown conversion
├── websearch/
│   └── active_tabs/         # Cached Markdown files
├── Cargo.toml
├── .env                     # Your API key (not in git)
└── README.md
```

## Dependencies

- `ratatui` - Terminal UI framework
- `crossterm` - Cross-platform terminal manipulation
- `tokio` - Async runtime
- `reqwest` - HTTP client
- `readability-js` - Content extraction
- `html-to-markdown-rs` - HTML to Markdown conversion
- `serde` - Serialization
- `anyhow` - Error handling

## Tips

- The app keeps search results visible until you perform a new search
- You can switch between Neovim and browser viewing without losing your place
- Cached pages load instantly on subsequent opens
- Multi-selection is great for research - open multiple relevant pages at once

## License

MIT