//! System Configuration module
//!
//! Manages system-level configuration with hierarchical overrides.

use serde::{Deserialize, Serialize};

/// Complete system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub ollama: OllamaConfig,
    pub session: SessionConfig,
    pub meeting: MeetingConfig,
    pub prompt: PromptConfig,
    pub security: SecurityConfig,
    pub logging: LoggingConfig,
}

/// Ollama connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub endpoint: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub retry_attempts: u32,
    pub streaming_enabled: bool,
}

/// Session persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub persistence_interval_secs: u64,
    pub storage_location: String,
    pub max_history_size: usize,
}

/// Meeting orchestration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingConfig {
    pub turn_limit: u32,
    pub conclusion_detection: bool,
    pub conflict_surfacing: bool,
}

/// Prompt assembly configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    pub template_strategy: String,
    pub max_context_tokens: usize,
    pub history_prioritization: String,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub manipulation_detection: bool,
    pub manipulation_logging: bool,
    pub response_validation: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub location: String,
}

impl SystemConfig {
    /// Create default configuration
    pub fn default_config() -> Self {
        Self {
            ollama: OllamaConfig {
                endpoint: "http://localhost:11434".to_string(),
                default_model: "llama3.1:8b".to_string(),
                timeout_secs: 30,
                retry_attempts: 3,
                streaming_enabled: true,
            },
            session: SessionConfig {
                persistence_interval_secs: 60,
                storage_location: "./sessions".to_string(),
                max_history_size: 1000,
            },
            meeting: MeetingConfig {
                turn_limit: 10,
                conclusion_detection: true,
                conflict_surfacing: true,
            },
            prompt: PromptConfig {
                template_strategy: "default".to_string(),
                max_context_tokens: 4096,
                history_prioritization: "recent-first".to_string(),
            },
            security: SecurityConfig {
                manipulation_detection: true,
                manipulation_logging: true,
                response_validation: true,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                location: "./logs".to_string(),
            },
        }
    }

    /// Load configuration with CLI overrides
    pub fn load_with_overrides(
        ollama_url: String,
        model_override: Option<String>,
    ) -> Self {
        let mut config = Self::default_config();

        config.ollama.endpoint = ollama_url;

        if let Some(model) = model_override {
            config.ollama.default_model = model;
        }

        // Check environment variables
        if let Ok(endpoint) = std::env::var("DOMAIN_EXPERT_OLLAMA_ENDPOINT") {
            config.ollama.endpoint = endpoint;
        }
        if let Ok(model) = std::env::var("DOMAIN_EXPERT_OLLAMA_MODEL") {
            config.ollama.default_model = model;
        }
        if let Ok(timeout) = std::env::var("DOMAIN_EXPERT_OLLAMA_TIMEOUT") {
            if let Ok(t) = timeout.parse() {
                config.ollama.timeout_secs = t;
            }
        }

        config
    }

    /// Validate the configuration
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.ollama.endpoint.is_empty() {
            errors.push("Ollama endpoint cannot be empty".to_string());
        }
        if self.ollama.default_model.is_empty() {
            errors.push("Ollama default model cannot be empty".to_string());
        }
        if self.ollama.timeout_secs == 0 {
            errors.push("Ollama timeout must be at least 1 second".to_string());
        }
        if self.session.storage_location.is_empty() {
            errors.push("Session storage location cannot be empty".to_string());
        }
        if self.meeting.turn_limit < 2 {
            errors.push("Meeting turn limit must be at least 2".to_string());
        }

        errors
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self::default_config()
    }
}
