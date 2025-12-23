//! Startpage search integration via HTML scraping
//!
//! Improved version with robust multi-strategy parsing.
//!
//! Features:
//! - Multiple fallback parsing strategies
//! - Better error handling and logging
//! - More reliable URL extraction
//! - Flexible selector matching

use anyhow::{Context, Result};
use scraper::{Html, Selector, ElementRef};
use std::collections::HashSet;

use crate::globals::get_http_client;
use crate::search::SearchResult;

/// Maximum number of search results to fetch
pub const MAX_RESULTS: usize = 10;

/// Minimum title length to consider valid
const MIN_TITLE_LENGTH: usize = 5;

/// Maximum title length to avoid capturing navigation elements
const MAX_TITLE_LENGTH: usize = 200;

/// Perform search using Startpage
pub async fn startpage_search(query: &str) -> Result<Vec<SearchResult>> {
    let client = get_http_client();

    // Startpage search URL with English language
    let url = format!(
        "https://www.startpage.com/sp/search?q={}&language=english",
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Accept-Encoding", "gzip, deflate, br")
        .header("DNT", "1")
        .header("Connection", "keep-alive")
        .header("Upgrade-Insecure-Requests", "1")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to send search request to Startpage")?;

    if !response.status().is_success() {
        anyhow::bail!("Startpage returned status: {}", response.status());
    }

    let html = response
        .text()
        .await
        .context("Failed to read Startpage response")?;

    parse_startpage_html(&html)
}

/// Parse Startpage HTML results page using multiple strategies
fn parse_startpage_html(html: &str) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html);

    // Try strategies in order of reliability
    let strategies: Vec<Box<dyn Fn(&Html) -> Option<Vec<SearchResult>>>> = vec![
        Box::new(strategy_structured_results),
        Box::new(strategy_link_clustering),
        Box::new(strategy_generic_links),
    ];

    for (_idx, strategy) in strategies.iter().enumerate() {
        if let Some(results) = strategy(&document) {
            if !results.is_empty() {
                return Ok(results);
            }
        }
    }

    anyhow::bail!(
        "All parsing strategies failed. Startpage's HTML structure may have changed significantly."
    )
}

/// Strategy 1: Look for structured result containers
///
/// This tries to find dedicated result containers with predictable structure.
fn strategy_structured_results(document: &Html) -> Option<Vec<SearchResult>> {
    // Common class patterns for Startpage result containers
    let container_patterns = vec![
        ".w-gl__result",           // Modern layout
        ".result",                 // Classic layout
        "article",                 // Semantic HTML
        "[data-testid*='result']", // Test ID pattern
        ".web-result",             // Alternative naming
    ];

    for pattern in container_patterns {
        if let Ok(container_sel) = Selector::parse(pattern) {
            let containers: Vec<_> = document.select(&container_sel).collect();
            
            if containers.len() >= 2 { // At least 2 results to be confident
                let results = extract_from_containers(&containers);
                if !results.is_empty() {
                    return Some(results);
                }
            }
        }
    }

    None
}

/// Extract results from result containers
fn extract_from_containers(containers: &[ElementRef]) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();

    for container in containers.iter().take(MAX_RESULTS * 2) {
        // Try multiple selector combinations for title link
        let title_link = find_title_link(container);
        
        if let Some((title, url)) = title_link {
            // Skip duplicates early
            if seen_urls.contains(&url) {
                continue;
            }
            
            if !is_valid_result(&title, &url) {
                continue;
            }

            seen_urls.insert(url.clone());

            // Find description in various ways
            let description = find_description(container)
                .unwrap_or_else(|| "No description available".to_string());

            results.push(SearchResult {
                title,
                url,
                description,
            });

            if results.len() >= MAX_RESULTS {
                break;
            }
        }
    }

    results
}

/// Find title link within a container using multiple selector patterns
fn find_title_link(container: &ElementRef) -> Option<(String, String)> {
    // Strategy 1: Look for heading-wrapped links first (most reliable)
    let heading_link_patterns = vec![
        "h2 a[href^='http']",
        "h3 a[href^='http']",
        "h1 a[href^='http']",
    ];

    for pattern in heading_link_patterns {
        if let Ok(selector) = Selector::parse(pattern) {
            if let Some(link_elem) = container.select(&selector).next() {
                if let Some((title, url)) = extract_title_url(link_elem) {
                    return Some((title, url));
                }
            }
        }
    }

    // Strategy 2: Look for classed links
    let class_link_patterns = vec![
        "a.w-gl__result-title",
        "a.result-link",
        "a.result-title",
        "a[class*='title']",
    ];

    for pattern in class_link_patterns {
        if let Ok(selector) = Selector::parse(pattern) {
            if let Some(link_elem) = container.select(&selector).next() {
                if let Some((title, url)) = extract_title_url(link_elem) {
                    return Some((title, url));
                }
            }
        }
    }

    // Strategy 3: Any http link (least reliable)
    if let Ok(selector) = Selector::parse("a[href^='http']") {
        if let Some(link_elem) = container.select(&selector).next() {
            if let Some((title, url)) = extract_title_url(link_elem) {
                return Some((title, url));
            }
        }
    }

    None
}

/// Extract title and URL from a link element
fn extract_title_url(link_elem: ElementRef) -> Option<(String, String)> {
    let url = link_elem.value().attr("href")?;
    
    // Skip internal Startpage links
    if url.contains("startpage.com") && !url.starts_with("http") {
        return None;
    }

    // Try to get title from multiple sources, in order of preference:
    // 1. Parent heading element
    // 2. Link text itself
    // 3. Title attribute
    
    let mut title = String::new();

    // Try parent heading first
    if let Some(parent) = link_elem.parent() {
        if let Some(parent_elem) = ElementRef::wrap(parent) {
            let tag_name = parent_elem.value().name();
            if tag_name == "h1" || tag_name == "h2" || tag_name == "h3" {
                title = extract_clean_text(&parent_elem);
            }
        }
    }

    // Fallback to link text
    if title.is_empty() || title.len() < MIN_TITLE_LENGTH {
        title = extract_clean_text(&link_elem);
    }

    // Last resort: title attribute
    if title.is_empty() || title.len() < MIN_TITLE_LENGTH {
        if let Some(title_attr) = link_elem.value().attr("title") {
            title = title_attr.trim().to_string();
        }
    }

    // Final validation
    if title.is_empty() || title.len() < MIN_TITLE_LENGTH {
        return None;
    }

    Some((title, url.to_string()))
}

/// Extract clean text from element, excluding script/style/etc tags
fn extract_clean_text(elem: &ElementRef) -> String {
    let text = elem
        .descendants()
        .filter_map(|node| {
            // Skip script, style, noscript tags
            if let Some(element) = ElementRef::wrap(node) {
                let tag_name = element.value().name();
                if tag_name == "script" || tag_name == "style" || tag_name == "noscript" {
                    return None;
                }
            }
            
            // Get text nodes
            node.value().as_text().map(|t| t.text.as_ref())
        })
        .collect::<Vec<_>>()
        .join(" ");
    
    // Clean up whitespace
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Find description text within a container
fn find_description(container: &ElementRef) -> Option<String> {
    let desc_patterns = vec![
        ".w-gl__description",
        ".result-abstract",
        ".result-content",
        ".result__snippet",
        "p.description",
        ".snippet",
    ];

    for pattern in desc_patterns {
        if let Ok(selector) = Selector::parse(pattern) {
            if let Some(desc_elem) = container.select(&selector).next() {
                let text = extract_clean_text(&desc_elem);
                if !text.is_empty() && text.len() > 10 {
                    return Some(text);
                }
            }
        }
    }

    // Fallback: find any <p> tag
    if let Ok(p_selector) = Selector::parse("p") {
        for p_elem in container.select(&p_selector) {
            let text = extract_clean_text(&p_elem);
            if text.len() > 20 && text.len() < 500 {
                return Some(text);
            }
        }
    }

    None
}

/// Strategy 2: Link clustering approach
///
/// Groups links that appear close together and filters by quality.
fn strategy_link_clustering(document: &Html) -> Option<Vec<SearchResult>> {
    let link_selector = Selector::parse("a[href^='http']").ok()?;
    
    let mut link_groups: Vec<Vec<ElementRef>> = Vec::new();
    let mut current_group: Vec<ElementRef> = Vec::new();
    let mut last_depth = 0;

    // Cluster links by DOM proximity
    for link in document.select(&link_selector) {
        let url = link.value().attr("href")?;
        
        // Skip Startpage internal links
        if url.contains("startpage.com") || 
           url.contains("privacy") ||
           url.contains("settings") {
            continue;
        }

        // Calculate approximate DOM depth
        let depth = count_ancestors(&link);
        
        // Start new group if depth changes significantly
        if (depth as i32 - last_depth as i32).abs() > 2 && !current_group.is_empty() {
            link_groups.push(current_group.clone());
            current_group.clear();
        }

        current_group.push(link);
        last_depth = depth;
    }

    if !current_group.is_empty() {
        link_groups.push(current_group);
    }

    // Find the group that looks most like search results
    let best_group = link_groups.into_iter()
        .filter(|g| g.len() >= 3 && g.len() <= 20)
        .max_by_key(|g| g.len())?;

    extract_from_link_group(&best_group)
}

/// Extract results from a group of similar links
fn extract_from_link_group(links: &[ElementRef]) -> Option<Vec<SearchResult>> {
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();

    for link in links.iter().take(MAX_RESULTS * 2) {
        let url = link.value().attr("href")?.to_string();
        
        if seen_urls.contains(&url) {
            continue;
        }
        seen_urls.insert(url.clone());

        let title = extract_clean_text(link);
        
        if !is_valid_result(&title, &url) {
            continue;
        }

        // Try to find description near the link
        let description = find_nearby_description(link)
            .unwrap_or_else(|| "No description available".to_string());

        results.push(SearchResult {
            title,
            url,
            description,
        });

        if results.len() >= MAX_RESULTS {
            break;
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

/// Find description text near a link element
fn find_nearby_description(link: &ElementRef) -> Option<String> {
    // Try parent's next sibling
    if let Some(parent) = link.parent() {
        if let Some(next_sib) = parent.next_sibling() {
            if let Some(elem) = ElementRef::wrap(next_sib) {
                let text = extract_clean_text(&elem);
                if text.len() > 20 && text.len() < 500 {
                    return Some(text);
                }
            }
        }

        // Try to find <p> in parent
        if let Some(parent_elem) = ElementRef::wrap(parent) {
            if let Ok(p_sel) = Selector::parse("p") {
                for p in parent_elem.select(&p_sel) {
                    let text = extract_clean_text(&p);
                    if text.len() > 20 && text.len() < 500 {
                        return Some(text);
                    }
                }
            }
        }
    }

    None
}

/// Strategy 3: Generic link extraction with aggressive filtering
///
/// Last resort: find all external links and filter heavily.
fn strategy_generic_links(document: &Html) -> Option<Vec<SearchResult>> {
    let link_selector = Selector::parse("a[href^='http']").ok()?;
    
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();

    for link in document.select(&link_selector) {
        let url = link.value().attr("href")?.to_string();
        
        // Skip known patterns
        if url.contains("startpage.com") ||
           url.contains("privacy") ||
           url.contains("cookie") ||
           url.contains("terms") ||
           url.contains("login") ||
           url.contains("signup") ||
           seen_urls.contains(&url) {
            continue;
        }
        
        seen_urls.insert(url.clone());

        let title = extract_clean_text(&link);
        
        if !is_valid_result(&title, &url) {
            continue;
        }

        results.push(SearchResult {
            title,
            url,
            description: "No description available".to_string(),
        });

        if results.len() >= MAX_RESULTS {
            break;
        }
    }

    if results.len() >= 3 {
        Some(results)
    } else {
        None
    }
}

/// Check if title and URL combination looks like a valid search result
fn is_valid_result(title: &str, url: &str) -> bool {
    // Title validation
    if title.len() < MIN_TITLE_LENGTH || title.len() > MAX_TITLE_LENGTH {
        return false;
    }

    // Skip if title is just URL
    if title.starts_with("http") {
        return false;
    }

    // Skip if title is suspiciously repetitive (like "... ... ...")
    if title.chars().filter(|&c| c == '.').count() > title.len() / 3 {
        return false;
    }

    // Skip if title looks like CSS or code
    let css_indicators = ["{", "}", ":", ";", "px", "rem", "rgb", "rgba", "var("];
    let has_css = css_indicators.iter().any(|&indicator| title.contains(indicator));
    if has_css {
        return false;
    }

    // Skip if title has too many curly braces or semicolons (code-like)
    let special_count = title.chars().filter(|&c| c == '{' || c == '}' || c == ';').count();
    if special_count > 2 {
        return false;
    }

    // Skip if title is just whitespace or special characters
    let alphanumeric_count = title.chars().filter(|c| c.is_alphanumeric()).count();
    if alphanumeric_count < MIN_TITLE_LENGTH {
        return false;
    }

    // Skip navigation-like titles (exact match)
    let nav_keywords = ["home", "login", "sign in", "sign up", "privacy", "terms", "cookie", "settings"];
    let title_lower = title.to_lowercase();
    if nav_keywords.iter().any(|&kw| title_lower == kw) {
        return false;
    }

    // URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    // Skip common non-result domains
    let skip_domains = [
        "startpage.com",
        "facebook.com/login",
        "twitter.com/login",
        "linkedin.com/login",
    ];
    
    if skip_domains.iter().any(|&domain| url.contains(domain)) {
        return false;
    }

    true
}

/// Count ancestors of an element (approximate DOM depth)
fn count_ancestors(elem: &ElementRef) -> usize {
    let mut count = 0;
    let mut current = elem.parent();
    
    while current.is_some() {
        count += 1;
        current = current.and_then(|p| p.parent());
        
        // Prevent infinite loops
        if count > 100 {
            break;
        }
    }
    
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_result() {
        // Valid titles
        assert!(is_valid_result("Rust Programming Language", "https://rust-lang.org"));
        assert!(is_valid_result("Example Article Title", "https://example.com/article"));
        assert!(is_valid_result("Hello World", "https://example.com"));
        
        // Invalid - too short
        assert!(!is_valid_result("Hi", "https://example.com"));
        
        // Invalid - title is URL
        assert!(!is_valid_result("https://example.com", "https://example.com"));
        
        // Invalid - navigation
        assert!(!is_valid_result("Login", "https://example.com/login"));
        
        // Invalid - not URL
        assert!(!is_valid_result("Valid Title", "not-a-url"));
        
        // Invalid - only dots
        assert!(!is_valid_result("...........", "https://example.com"));
        
        // Invalid - no alphanumeric
        assert!(!is_valid_result("!!!!", "https://example.com"));
        
        // Invalid - CSS-like content
        assert!(!is_valid_result(".container { padding: 10px; }", "https://example.com"));
        assert!(!is_valid_result("color: rgb(255, 0, 0);", "https://example.com"));
        assert!(!is_valid_result("var(--primary-color)", "https://example.com"));
        
        // Invalid - code-like with many special chars
        assert!(!is_valid_result("{ a: 1; b: 2; c: 3; }", "https://example.com"));
    }

    #[test]
    fn test_max_results_constant() {
        assert_eq!(MAX_RESULTS, 10);
    }

    #[test]
    fn test_parse_simple_link_list() {
        let html = r#"
            <div>
                <a href="https://example.com">Example Domain</a>
                <a href="https://rust-lang.org">Rust Programming Language</a>
                <a href="https://github.com">GitHub Homepage</a>
            </div>
        "#;
        
        let doc = Html::parse_document(html);
        let results = strategy_generic_links(&doc);
        
        assert!(results.is_some());
        let results = results.unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].url, "https://example.com");
    }

    #[test]
    fn test_filters_startpage_links() {
        let html = r#"
            <div>
                <a href="https://example.com">Valid Result</a>
                <a href="https://www.startpage.com/privacy">Privacy Policy</a>
                <a href="https://rust-lang.org">Rust Language</a>
            </div>
        "#;
        
        let doc = Html::parse_document(html);
        let results = strategy_generic_links(&doc);
        
        assert!(results.is_some());
        let results = results.unwrap();
        assert_eq!(results.len(), 2);
        assert!(!results.iter().any(|r| r.url.contains("startpage")));
    }

    #[test]
    fn test_deduplication() {
        let html = r#"
            <div>
                <h2><a href="https://example.com">Example Domain</a></h2>
                <a href="https://example.com">Duplicate Link</a>
                <a href="https://rust-lang.org">Rust Language</a>
            </div>
        "#;
        
        let doc = Html::parse_document(html);
        let results = strategy_generic_links(&doc);
        
        assert!(results.is_some());
        let results = results.unwrap();
        
        // Should have only 2 results (duplicate filtered out)
        assert_eq!(results.len(), 2);
        
        // URLs should be unique
        let urls: Vec<&str> = results.iter().map(|r| r.url.as_str()).collect();
        assert_eq!(urls[0], "https://example.com");
        assert_eq!(urls[1], "https://rust-lang.org");
    }

    #[test]
    fn test_title_priority() {
        let html = r#"
            <div class="result">
                <h2><a href="https://example.com">Correct Title from H2</a></h2>
                <a href="https://example.com">Wrong duplicate link</a>
            </div>
        "#;
        
        let doc = Html::parse_document(html);
        
        if let Ok(container_sel) = Selector::parse(".result") {
            if let Some(container) = doc.select(&container_sel).next() {
                let title_link = find_title_link(&container);
                
                assert!(title_link.is_some());
                let (title, _) = title_link.unwrap();
                
                // Should prefer heading-wrapped link
                assert_eq!(title, "Correct Title from H2");
            }
        }
    }

    #[test]
    fn test_alphanumeric_validation() {
        // Valid - has enough alphanumeric chars
        assert!(is_valid_result("Hello World", "https://example.com"));
        
        // Invalid - mostly special chars
        assert!(!is_valid_result("...", "https://example.com"));
        assert!(!is_valid_result("!!!", "https://example.com"));
        assert!(!is_valid_result("---", "https://example.com"));
    }

    #[test]
    fn test_filters_css_content() {
        let html = r#"
            <div>
                <a href="https://example.com">Valid Title</a>
                <a href="https://bad.com"><style>.test { color: red; }</style>CSS Content</a>
                <a href="https://rust-lang.org">Rust Language</a>
            </div>
        "#;
        
        let doc = Html::parse_document(html);
        let results = strategy_generic_links(&doc);
        
        assert!(results.is_some());
        let results = results.unwrap();
        
        // Should filter out the link with CSS
        for result in &results {
            assert!(!result.title.contains("color:"));
            assert!(!result.title.contains("{"));
        }
    }

    #[test]
    fn test_extract_clean_text() {
        let html = r#"
            <div>
                <style>.hidden { display: none; }</style>
                <script>alert('test');</script>
                Good Title Text
                <noscript>No JS</noscript>
            </div>
        "#;
        
        let doc = Html::parse_document(html);
        if let Ok(sel) = Selector::parse("div") {
            if let Some(elem) = doc.select(&sel).next() {
                let text = extract_clean_text(&elem);
                
                // Should not contain CSS, script, or noscript content
                assert!(!text.contains("display"));
                assert!(!text.contains("alert"));
                assert!(!text.contains("No JS"));
                
                // Should contain actual text
                assert!(text.contains("Good Title Text"));
            }
        }
    }
}
