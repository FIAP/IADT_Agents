//! Ollama Integration module
//!
//! Handles communication with local Ollama LLM instances,
//! including streaming, retries, and error handling.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::time::Duration;

/// Errors from Ollama communication
#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("Cannot connect to Ollama at {endpoint}: {message}")]
    ConnectionFailed { endpoint: String, message: String },

    #[error("Ollama request timed out after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    #[error("Model '{model}' not found. Run: ollama pull {model}")]
    ModelNotFound { model: String },

    #[error("Ollama API error: {0}")]
    ApiError(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

/// Ollama API request body
#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

/// Ollama API response body
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GenerateResponse {
    response: String,
    #[serde(default)]
    done: bool,
}

/// Connection status check response
#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

/// Client for communicating with local Ollama instances
pub struct OllamaClient {
    client: Client,
    endpoint: String,
    timeout_secs: u64,
    retry_attempts: u32,
}

impl OllamaClient {
    /// Create a new Ollama client
    pub fn new(endpoint: &str, timeout_secs: u64, retry_attempts: u32) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            timeout_secs,
            retry_attempts,
        }
    }

    /// Check if Ollama is available and responding
    pub async fn check_connection(&self) -> Result<Vec<String>, OllamaError> {
        let url = format!("{}/api/tags", self.endpoint);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| OllamaError::ConnectionFailed {
                endpoint: self.endpoint.clone(),
                message: e.to_string(),
            })?;

        let tags: TagsResponse = response.json().await.map_err(|e| {
            OllamaError::ApiError(format!("Failed to parse Ollama response: {}", e))
        })?;

        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    /// Send a prompt to Ollama and get the complete response
    pub async fn generate(&self, model: &str, prompt: &str) -> Result<String, OllamaError> {
        let url = format!("{}/api/generate", self.endpoint);

        let request = GenerateRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let mut last_error = None;

        for attempt in 0..=self.retry_attempts {
            if attempt > 0 {
                let backoff = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                tracing::warn!(
                    "Retry attempt {} after {:?}",
                    attempt,
                    backoff
                );
                tokio::time::sleep(backoff).await;
            }

            match self.client.post(&url).json(&request).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<GenerateResponse>().await {
                            Ok(gen_response) => return Ok(gen_response.response),
                            Err(e) => {
                                last_error = Some(OllamaError::ApiError(format!(
                                    "Failed to parse response: {}",
                                    e
                                )));
                            }
                        }
                    } else {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();

                        if status.as_u16() == 404 || body.contains("not found") {
                            return Err(OllamaError::ModelNotFound {
                                model: model.to_string(),
                            });
                        }

                        last_error = Some(OllamaError::ApiError(format!(
                            "HTTP {} - {}",
                            status, body
                        )));
                    }
                }
                Err(e) if e.is_timeout() => {
                    last_error = Some(OllamaError::Timeout {
                        timeout_secs: self.timeout_secs,
                    });
                }
                Err(e) if e.is_connect() => {
                    return Err(OllamaError::ConnectionFailed {
                        endpoint: self.endpoint.clone(),
                        message: e.to_string(),
                    });
                }
                Err(e) => {
                    last_error = Some(OllamaError::Http(e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            OllamaError::ApiError("Unknown error".to_string())
        }))
    }
}
