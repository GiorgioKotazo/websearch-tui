//! Global shared resources for optimal performance
//!
//! This module provides singleton instances of expensive-to-create resources:
//! - HTTP client with optimized connection pooling and compression

use anyhow::Result;
use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

/// Global HTTP client - reuses connections across requests
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Get or create the global HTTP client
///
/// Features:
/// - Connection pooling (reuses TCP connections)
/// - Gzip/Brotli decompression (reduces bandwidth ~4x)
/// - TCP and HTTP/2 keepalive
/// - Reasonable timeouts
/// - Proper User-Agent
pub fn get_http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            // Timeouts
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(15))
            // Connection pooling - OPTIMIZED
            .pool_max_idle_per_host(15) // Up from 10
            .pool_idle_timeout(Duration::from_secs(120)) // Up from 90
            // TCP/HTTP keepalive - NEW
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .http2_keep_alive_interval(Some(Duration::from_secs(30)))
            .http2_keep_alive_timeout(Duration::from_secs(10))
            // Compression - reduces traffic ~4x
            .gzip(true)
            .brotli(true)
            // User agent (some sites block requests without it)
            .user_agent(concat!(
                "Mozilla/5.0 (compatible; websearch-tui/",
                env!("CARGO_PKG_VERSION"),
                "; +https://github.com/user/websearch-tui)"
            ))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// Initialize all global resources upfront
///
/// Call this at startup to avoid initialization delays during first use.
pub fn init_globals() -> Result<()> {
    // Force initialization of HTTP client
    let _ = get_http_client();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_client_singleton() {
        let client1 = get_http_client();
        let client2 = get_http_client();
        assert!(std::ptr::eq(client1, client2));
    }
}