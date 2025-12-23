//! SearXNG search integration with fallback mechanism
//!
//! Uses a curated list of reliable public SearXNG instances with:
//! - Random instance selection for load distribution
//! - Automatic fallback to other instances on failure
//! - Multiple engine strategy (not just Google to avoid blocks)
//! - JSON API for structured responses

use anyhow::{Context, Result};
use rand::{rngs::StdRng, SeedableRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};

use crate::globals::get_http_client;
use crate::search::SearchResult;

/// Maximum number of search results to fetch
pub const MAX_RESULTS: usize = 10;

/// Maximum retry attempts across different instances
const MAX_RETRY_ATTEMPTS: usize = 5;

/// Curated list of reliable SearXNG public instances
/// 
/// These instances have been tested and confirmed to:
/// - Support JSON API
/// - Have reasonable uptime
/// - Allow web search without heavy rate limiting
const SEARXNG_INSTANCES: &[&str] = &[
    "https://search.atlas.engineer",
    "https://searx.be",
    "https://searx.tiekoetter.com",
    "https://search.ononoki.org",
    "https://search.bus-hit.me",
];

/// SearXNG JSON response structure
#[derive(Debug, Serialize, Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngResult>,
    #[serde(default)]
    number_of_results: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearxngResult {
    title: String,
    url: String,
    #[serde(default)]
    content: Option<String>,
}

/// Perform search using SearXNG with fallback mechanism
///
/// Strategy:
/// 1. Don't specify engines (let SearXNG aggregate from all available)
/// 2. This avoids Google-specific rate limiting
/// 3. SearXNG will use whatever engines are working for that instance
/// 4. Results are still high quality due to aggregation
pub async fn searxng_search(query: &str) -> Result<Vec<SearchResult>> {
    let client = get_http_client();
    
    // Shuffle instances for random selection
    let mut instances = SEARXNG_INSTANCES.to_vec();
    let mut rng = StdRng::from_entropy();
    instances.shuffle(&mut rng);

    let mut last_error = None;
    let attempts = MAX_RETRY_ATTEMPTS.min(instances.len());

    // Try multiple instances until one succeeds
    for instance_url in instances.iter().take(attempts) {
        // Try with default engines first (better success rate)
        match try_search_instance(&client, instance_url, query, None).await {
            Ok(results) => {
                if !results.is_empty() {
                    return Ok(results);
                }
            }
            Err(e) => {
                // Store error but continue trying
                last_error = Some(e);
            }
        }
        
        // If default engines failed, try explicitly with common engines
        match try_search_instance(&client, instance_url, query, Some("duckduckgo,bing")).await {
            Ok(results) => {
                if !results.is_empty() {
                    return Ok(results);
                }
            }
            Err(e) => {
                last_error = Some(e);
            }
        }
    }

    // All instances failed
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!(
        "All SearXNG instances failed. This may be due to rate limiting or temporary unavailability. Try DuckDuckGo (Ctrl+D) instead."
    )))
}

/// Try searching a specific SearXNG instance
async fn try_search_instance(
    client: &reqwest::Client,
    instance_url: &str,
    query: &str,
    engines: Option<&str>,
) -> Result<Vec<SearchResult>> {
    // Build search URL
    let mut url = format!(
        "{}/search?q={}&format=json&categories=general",
        instance_url,
        urlencoding::encode(query)
    );
    
    // Add engines parameter if specified
    if let Some(eng) = engines {
        url.push_str(&format!("&engines={}", eng));
    }

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Accept-Language", "en-US,en;q=0.9")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .context(format!("Failed to connect to {}", instance_url))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "{} returned status: {}",
            instance_url,
            response.status()
        );
    }

    let text = response
        .text()
        .await
        .context("Failed to read response body")?;

    // Debug: Log response for troubleshooting
    if text.len() < 100 {
        eprintln!("⚠️  Short response from {}: {}", instance_url, text);
    }

    // Parse JSON response with better error handling
    let searxng_response: SearxngResponse = serde_json::from_str(&text)
        .context(format!("Failed to parse JSON from {}. Response length: {} bytes", 
            instance_url, text.len()))?;

    // Convert to our SearchResult format
    let results: Vec<SearchResult> = searxng_response
        .results
        .into_iter()
        .take(MAX_RESULTS)
        .filter(|r| !r.title.is_empty() && !r.url.is_empty())
        .map(|r| SearchResult {
            title: r.title,
            url: r.url,
            description: r.content.unwrap_or_else(|| String::from("No description")),
        })
        .collect();

    if results.is_empty() {
        anyhow::bail!("No results found from {}", instance_url);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instances_list() {
        assert!(!SEARXNG_INSTANCES.is_empty());
        assert!(SEARXNG_INSTANCES.len() >= 5);
    }

    #[test]
    fn test_max_results_constant() {
        assert_eq!(MAX_RESULTS, 10);
    }

    #[tokio::test]
    async fn test_url_encoding() {
        let query = "rust programming language";
        let encoded = urlencoding::encode(query);
        assert!(encoded.contains("rust"));
    }
}