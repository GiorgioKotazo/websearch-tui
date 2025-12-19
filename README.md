# websearch-tui

⚡ Lightning-fast terminal web search with Neovim integration

## Features

- 🔍 **Fast Search** - Brave Search API integration
- 🚀 **Background Prefetching** - All 10 results are downloaded and processed in parallel immediately after search
- 📄 **Clean Markdown** - Mozilla Readability extracts main content, removing ads and navigation
- 📝 **Neovim Integration** - Open pages instantly in Neovim (pages are already prefetched!)
- 🎯 **Vim-like Navigation** - `j/k`, `gg/G`, and more

## Performance Optimizations

This version includes significant performance improvements over the original:

### 1. Global HTTP Client with Connection Pooling
```
Before: New HTTP client created for each request (~100-200ms overhead)
After:  Single client reused, connections kept alive
Savings: ~150ms per request
```

### 2. Global Readability Instance
```
Before: New Readability instance for each page (~30ms initialization)
After:  Single instance reused
Savings: ~30ms per page
```

### 3. Parallel Background Prefetching
```
Before: Page downloaded only when user presses Enter (3-5 seconds wait)
After:  All 10 pages downloaded in parallel immediately after search
Result: Instant page opening (0ms wait)
```

### 4. HTTP Compression
```
Before: Raw HTML downloaded
After:  gzip/brotli compression enabled
Savings: 3-5x bandwidth reduction
```

### Expected Performance

| Operation | Before | After | Improvement |
|-----------|--------|-------|-------------|
| Search | 2-3s | 1-2s | 1.5x faster |
| First page open | 3-5s | ~0ms* | **Instant** |
| Subsequent pages | 3-5s | ~0ms* | **Instant** |

*Pages are prefetched in background while you browse results

## Installation

```bash
# Clone the repository
git clone https://github.com/user/websearch-tui
cd websearch-tui

# Build with optimizations
cargo build --release

# Install
cargo install --path .
```

## Configuration

Create a `.env` file or set environment variables:

```bash
# Required: Brave Search API key
# Get one at: https://brave.com/search/api/
BRAVE_SEARCH_API_KEY=your_api_key_here
```

## Usage

```bash
websearch-tui
```

### Keyboard Shortcuts

#### Search Mode
| Key | Action |
|-----|--------|
| `Enter` | Start search |
| `Esc` | Clear input |
| `Ctrl+Q` | Quit |

#### Results Mode
| Key | Action |
|-----|--------|
| `j` / `↓` | Next result |
| `k` / `↑` | Previous result |
| `gg` | First result |
| `G` | Last result |
| `Tab` | Toggle selection |
| `Enter` | Open in Neovim |
| `Ctrl+B` | Open in browser |
| `Esc` | New search |
| `Ctrl+Q` | Quit |

## Directory Structure

```
websearch/
├── current_search/     # Prefetched pages for current search
│   ├── 01_Article_Title.md
│   ├── 02_Another_Page.md
│   └── ...
└── active_tabs/        # Pages opened in Neovim
    └── 01_Article_Title.md
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      main.rs                            │
│                   (Event Loop)                          │
└─────────────────────────┬───────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
    ┌──────────┐   ┌──────────┐   ┌──────────────┐
    │  app.rs  │   │  ui.rs   │   │ prefetch.rs  │
    │  (State) │   │  (TUI)   │   │ (Background) │
    └────┬─────┘   └──────────┘   └──────┬───────┘
         │                               │
         │         ┌─────────────────────┘
         │         │
         ▼         ▼
    ┌─────────────────────────────────────┐
    │           globals.rs                 │
    │  ┌─────────────┐ ┌────────────────┐ │
    │  │ HTTP Client │ │  Readability   │ │
    │  │   (Pool)    │ │   (Instance)   │ │
    │  └─────────────┘ └────────────────┘ │
    └─────────────────────────────────────┘
```

## How Prefetching Works

1. User enters search query
2. App fetches 10 results from Brave Search
3. **Immediately** spawns 5 concurrent tasks to download & process all pages
4. User sees results list with progress bar
5. As each page completes, it's saved to `current_search/`
6. When user presses Enter:
   - File is **moved** (not copied) from `current_search/` to `active_tabs/`
   - Neovim opens instantly (file already exists!)

## Dependencies

- **tokio** - Async runtime
- **ratatui** - Terminal UI
- **reqwest** - HTTP client with connection pooling
- **readability-js** - Mozilla Readability via QuickJS
- **html-to-markdown-rs** - HTML to Markdown conversion

## License

MIT

## Contributing

Contributions welcome! Please feel free to submit issues and PRs.

## Acknowledgments

- [Mozilla Readability](https://github.com/mozilla/readability) - Content extraction algorithm
- [Brave Search](https://brave.com/search/api/) - Search API
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
