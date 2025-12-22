//! DuckDuckGo search integration via HTML scraping
//!
//! Uses DuckDuckGo's HTML-only interface (html.duckduckgo.com) which is:
//! - Lightweight and fast (no JavaScript required)
//! - Scraping-friendly (DuckDuckGo encourages use of their data)
//! - Privacy-focused (no tracking, no personalization)
//!
//! This approach uses the existing HTTP client for optimal performance.

use anyhow::{Context, Result};
use scraper::{Html, Selector};

use crate::globals::get_http_client;
use crate::search::SearchResult;

/// Maximum number of search results to fetch
pub const MAX_RESULTS: usize = 10;

/// Perform search using DuckDuckGo HTML interface
///
/// Uses the html.duckduckgo.com static interface which is:
/// - Fast and lightweight (no JavaScript)
/// - Scraping-friendly
/// - Returns up to MAX_RESULTS results
pub async fn duckduckgo_search(query: &str) -> Result<Vec<SearchResult>> {
    let client = get_http_client();

    // Use DuckDuckGo's HTML-only interface
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .header("Accept", "text/html")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .context("Failed to send search request to DuckDuckGo")?;

    if !response.status().is_success() {
        anyhow::bail!("DuckDuckGo returned status: {}", response.status());
    }

    let html = response
        .text()
        .await
        .context("Failed to read DuckDuckGo response")?;

    parse_duckduckgo_html(&html)
}

/// Parse DuckDuckGo HTML results page
///
/// Extracts title, URL, and description from search results.
/// DuckDuckGo's HTML structure uses:
/// - Results are in <div class="result">
/// - Title and URL are in <a class="result__a">
/// - Description is in <a class="result__snippet">
fn parse_duckduckgo_html(html: &str) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html);

    // Selectors for DuckDuckGo HTML structure
    let result_selector = Selector::parse(".result")
        .map_err(|e| anyhow::anyhow!("Invalid result selector: {:?}", e))?;
    
    let title_selector = Selector::parse(".result__a")
        .map_err(|e| anyhow::anyhow!("Invalid title selector: {:?}", e))?;
    
    let snippet_selector = Selector::parse(".result__snippet")
        .map_err(|e| anyhow::anyhow!("Invalid snippet selector: {:?}", e))?;

    let mut results = Vec::new();

    for result_elem in document.select(&result_selector).take(MAX_RESULTS) {
        // Extract title and URL from the title link
        let title_elem = match result_elem.select(&title_selector).next() {
            Some(elem) => elem,
            None => continue,
        };

        // Get title text
        let title = title_elem
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();

        if title.is_empty() {
            continue;
        }

        // Get URL from href attribute
        let href = match title_elem.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        // DuckDuckGo uses redirect URLs like //duckduckgo.com/l/?uddg=<encoded_url>&rut=...
        let url = if href.starts_with("//duckduckgo.com/l/?") || href.starts_with("/l/?") {
            // Extract the uddg parameter
            if let Some(uddg_start) = href.find("uddg=") {
                let after_uddg = &href[uddg_start + 5..];
                // Find the end of the URL (next & or end of string)
                let url_end = after_uddg.find('&').unwrap_or(after_uddg.len());
                let encoded_url = &after_uddg[..url_end];
                
                // Decode the URL
                match urlencoding::decode(encoded_url) {
                    Ok(decoded) => decoded.to_string(),
                    Err(_) => continue,
                }
            } else {
                continue;
            }
        } else if href.starts_with("http://") || href.starts_with("https://") {
            // Direct URL (less common but possible)
            href.to_string()
        } else {
            continue;
        };

        if url.is_empty() {
            continue;
        }

        // Extract description/snippet
        let description = result_elem
            .select(&snippet_selector)
            .next()
            .map(|elem| {
                elem.text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string()
            })
            .unwrap_or_else(|| String::from("No description"));

        results.push(SearchResult {
            title,
            url,
            description,
        });
    }

    if results.is_empty() {
        anyhow::bail!("No results found or failed to parse DuckDuckGo HTML. The page structure may have changed.");
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sample_html() {
        let sample_html = r#"
            <div class="result">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=123">Example Title</a>
                <a class="result__snippet">Example description</a>
            </div>
            <div class="result">
                <a class="result__a" href="/l/?uddg=https%3A%2F%2Ftest.com">Test Page</a>
                <a class="result__snippet">Test description</a>
            </div>
        "#;

        let results = parse_duckduckgo_html(sample_html).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Example Title");
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(results[1].url, "https://test.com");
    }

    #[tokio::test]
    async fn test_max_results_constant() {
        assert_eq!(MAX_RESULTS, 10);
    }
}