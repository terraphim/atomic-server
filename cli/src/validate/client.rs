//! HTTP client for Atomic Data server communication.
//!
//! Provides retry logic, progress reporting, and robust error handling for server operations.

use atomic_lib::{
    agents::Agent,
    client::{fetch_resource, get_authentication_headers, post_commit},
    commit::Commit,
    errors::AtomicResult,
    parse::{parse_json_ad_string, ParseOpts},
    storelike::{ResourceResponse, Storelike},
    Resource,
};
use std::time::Duration;

/// Configuration for HTTP client operations
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Maximum number of retry attempts
    pub max_retries: usize,
    /// Base delay between retries (exponential backoff)
    pub retry_delay_ms: u64,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Whether to validate SSL certificates
    pub validate_ssl: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            timeout_secs: 30,
            validate_ssl: true,
        }
    }
}

/// HTTP client for Atomic Data operations with retry logic.
pub struct AtomicClient {
    config: ClientConfig,
    agent: Option<Agent>,
    /// Statistics about operations
    pub stats: ClientStats,
}

/// Statistics about client operations
#[derive(Debug, Clone, Default)]
pub struct ClientStats {
    pub resources_fetched: usize,
    pub commits_posted: usize,
    pub bytes_downloaded: usize,
    pub bytes_uploaded: usize,
    pub retries_attempted: usize,
    pub errors_encountered: usize,
}

impl AtomicClient {
    /// Create a new HTTP client with default configuration.
    pub fn new(agent: Option<Agent>) -> Self {
        Self {
            config: ClientConfig::default(),
            agent,
            stats: ClientStats::default(),
        }
    }

    /// Create a new HTTP client with custom configuration.
    pub fn with_config(agent: Option<Agent>, config: ClientConfig) -> Self {
        Self {
            config,
            agent,
            stats: ClientStats::default(),
        }
    }

    /// Fetch a resource from a server with retry logic.
    pub fn fetch_resource_with_retry(
        &mut self,
        subject: &str,
        store: &impl Storelike,
    ) -> AtomicResult<ResourceResponse> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                // Exponential backoff
                let delay = self.config.retry_delay_ms * 2u64.pow(attempt as u32 - 1);
                std::thread::sleep(Duration::from_millis(delay));
                self.stats.retries_attempted += 1;
            }

            match fetch_resource(subject, store, self.agent.as_ref()) {
                Ok(response) => {
                    self.stats.resources_fetched += 1;
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);
                    self.stats.errors_encountered += 1;

                    // Check if error is retryable
                    if !self.is_retryable_error(&last_error.as_ref().unwrap().to_string()) {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| "Unknown error".into()))
    }

    /// Fetch multiple resources with progress reporting.
    pub fn fetch_resources_batch(
        &mut self,
        subjects: &[String],
        store: &impl Storelike,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> Vec<AtomicResult<ResourceResponse>> {
        let total = subjects.len();
        let mut results = Vec::with_capacity(total);

        for (i, subject) in subjects.iter().enumerate() {
            if let Some(callback) = progress_callback {
                callback(i + 1, total);
            }

            results.push(self.fetch_resource_with_retry(subject, store));
        }

        results
    }

    /// Post a commit to a server with retry logic.
    pub fn post_commit_with_retry(
        &mut self,
        commit: &Commit,
        store: &impl Storelike,
    ) -> AtomicResult<()> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = self.config.retry_delay_ms * 2u64.pow(attempt as u32 - 1);
                std::thread::sleep(Duration::from_millis(delay));
                self.stats.retries_attempted += 1;
            }

            match post_commit(commit, store) {
                Ok(()) => {
                    self.stats.commits_posted += 1;
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    self.stats.errors_encountered += 1;

                    if !self.is_retryable_error(&last_error.as_ref().unwrap().to_string()) {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| "Unknown error".into()))
    }

    /// Post multiple commits with progress reporting.
    pub fn post_commits_batch(
        &mut self,
        commits: &[Commit],
        store: &impl Storelike,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> Vec<AtomicResult<()>> {
        let total = commits.len();
        let mut results = Vec::with_capacity(total);

        for (i, commit) in commits.iter().enumerate() {
            if let Some(callback) = progress_callback {
                callback(i + 1, total);
            }

            results.push(self.post_commit_with_retry(commit, store));
        }

        results
    }

    /// Fetch raw JSON-AD body with retry logic.
    pub fn fetch_json_ad_body(&mut self, url: &str) -> AtomicResult<String> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = self.config.retry_delay_ms * 2u64.pow(attempt as u32 - 1);
                std::thread::sleep(Duration::from_millis(delay));
                self.stats.retries_attempted += 1;
            }

            match self.fetch_body_internal(url, atomic_lib::parse::JSON_AD_MIME) {
                Ok(body) => {
                    self.stats.bytes_downloaded += body.len();
                    return Ok(body);
                }
                Err(e) => {
                    last_error = Some(e);
                    self.stats.errors_encountered += 1;

                    if !self.is_retryable_error(&last_error.as_ref().unwrap().to_string()) {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| "Unknown error".into()))
    }

    /// Internal method to fetch body with authentication.
    fn fetch_body_internal(&self, url: &str, content_type: &str) -> AtomicResult<String> {
        if !url.starts_with("http") {
            return Err(format!("URL must start with http: {}", url).into());
        }

        let client = ureq::builder()
            .timeout(Duration::from_secs(self.config.timeout_secs))
            .build();

        let mut req = client.get(url);
        if let Some(agent) = &self.agent {
            let headers = get_authentication_headers(url, agent)?;
            for (key, value) in headers {
                req = req.set(&key, &value);
            }
        }

        let resp = match req.set("Accept", content_type).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(status, response)) => {
                let body = response
                    .into_string()
                    .unwrap_or_else(|_| "<failed to read body>".to_string());
                return Err(format!("HTTP {}: {}", status, body).into());
            }
            Err(e) => return Err(format!("Network error: {}", e).into()),
        };

        if resp.status() != 200 {
            return Err(format!("HTTP status {}", resp.status()).into());
        }

        resp.into_string()
            .map_err(|e| format!("Failed to read response: {}", e).into())
    }

    /// Check if an error is retryable.
    fn is_retryable_error(&self, error: &str) -> bool {
        let error_lower = error.to_lowercase();

        // Retryable errors
        error_lower.contains("timeout")
            || error_lower.contains("connection")
            || error_lower.contains("network")
            || error_lower.contains("dns")
            || error_lower.contains("temporary")
            || error_lower.contains("503")
            || error_lower.contains("502")
            || error_lower.contains("500")
    }

    /// Test server connectivity.
    pub fn test_connection(&mut self, server_url: &str) -> AtomicResult<ServerInfo> {
        let drive_url = format!("{}/", server_url.trim_end_matches('/'));

        let start = std::time::Instant::now();
        let body = self.fetch_json_ad_body(&drive_url)?;
        let latency_ms = start.elapsed().as_millis() as u64;

        Ok(ServerInfo {
            url: server_url.to_string(),
            reachable: true,
            latency_ms,
            response_size: body.len(),
        })
    }

    /// Get the current agent.
    pub fn agent(&self) -> Option<&Agent> {
        self.agent.as_ref()
    }

    /// Get mutable reference to stats.
    pub fn stats_mut(&mut self) -> &mut ClientStats {
        &mut self.stats
    }
}

/// Information about a server.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub url: String,
    pub reachable: bool,
    pub latency_ms: u64,
    pub response_size: usize,
}

/// Batch operation result
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub errors: Vec<(String, String)>, // (subject, error)
}

impl BatchResult {
    pub fn new() -> Self {
        Self {
            total: 0,
            successful: 0,
            failed: 0,
            errors: Vec::new(),
        }
    }

    pub fn add_success(&mut self) {
        self.total += 1;
        self.successful += 1;
    }

    pub fn add_failure(&mut self, subject: String, error: String) {
        self.total += 1;
        self.failed += 1;
        self.errors.push((subject, error));
    }

    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.successful as f64 / self.total as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomic_lib::Store;

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_atomic_client_creation() {
        let client = AtomicClient::new(None);
        assert_eq!(client.stats.resources_fetched, 0);
        assert_eq!(client.stats.commits_posted, 0);
    }

    #[test]
    fn test_is_retryable_error() {
        let client = AtomicClient::new(None);

        // Retryable errors
        assert!(client.is_retryable_error("Connection timeout"));
        assert!(client.is_retryable_error("Network error"));
        assert!(client.is_retryable_error("DNS failure"));
        assert!(client.is_retryable_error("HTTP 503 Service Unavailable"));

        // Non-retryable errors
        assert!(!client.is_retryable_error("404 Not Found"));
        assert!(!client.is_retryable_error("Invalid JSON"));
        assert!(!client.is_retryable_error("Permission denied"));
    }

    #[test]
    fn test_batch_result() {
        let mut result = BatchResult::new();

        result.add_success();
        result.add_success();
        result.add_failure("http://example.com/bad".to_string(), "404".to_string());

        assert_eq!(result.total, 3);
        assert_eq!(result.successful, 2);
        assert_eq!(result.failed, 1);
        assert!((result.success_rate() - 66.66).abs() < 1.0);
    }

    #[test]
    fn test_client_with_custom_config() {
        let config = ClientConfig {
            max_retries: 5,
            retry_delay_ms: 500,
            timeout_secs: 60,
            validate_ssl: false,
        };

        let client = AtomicClient::with_config(None, config.clone());
        assert_eq!(client.config.max_retries, 5);
        assert_eq!(client.config.retry_delay_ms, 500);
    }
}
