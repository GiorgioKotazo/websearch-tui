use reqwest::Client;
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};

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
pub async fn brave_search(api_key: &str, query: &str) -> Result<Vec<SearchResult>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}", 
        urlencoding::encode(query)
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