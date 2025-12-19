//! Brave Search API integration
//!
//! Uses the global HTTP client for connection pooling and reuse.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::globals::get_http_client;

/// Maximum number of search results to fetch
pub const MAX_RESULTS: usize = 10;

/// Search result from Brave API
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BraveSearchResponse {
    web: Option<WebResults>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WebResults {
    results: Vec<BraveResult>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    description: Option<String>,
}

/// Perform search using Brave Search API
///
/// Uses the global HTTP client with connection pooling.
/// Returns up to MAX_RESULTS results.
pub async fn brave_search(api_key: &str, query: &str) -> Result<Vec<SearchResult>> {
    let client = get_http_client();

    // Request exactly MAX_RESULTS
    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
        urlencoding::encode(query),
        MAX_RESULTS
    );

    let response = client
        .get(&url)
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to send search request")?;

    if !response.status().is_success() {
        anyhow::bail!("API returned status: {}", response.status());
    }

    let search_response: BraveSearchResponse = response
        .json()
        .await
        .context("Failed to parse search response")?;

    let results = search_response
        .web
        .map(|web| {
            web.results
                .into_iter()
                .take(MAX_RESULTS) // Ensure we don't exceed limit
                .map(|r| SearchResult {
                    title: r.title,
                    url: r.url,
                    description: r.description.unwrap_or_else(|| String::from("No description")),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_results_constant() {
        assert_eq!(MAX_RESULTS, 10);
    }
}
