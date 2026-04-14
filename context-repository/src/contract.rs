//! Context Contract specification (v1.0.0)
//!
//! Defines the interface between System Repository and Context Repository.
//! Includes contract versioning, directory structure, and validation rules.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Semantic version for Context Contract
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemanticVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Check if this version is compatible with another version.
    /// Compatible means same major version and this version >= other.
    pub fn is_compatible_with(&self, other: &SemanticVersion) -> bool {
        self.major == other.major
            && (self.minor > other.minor
                || (self.minor == other.minor && self.patch >= other.patch))
    }

    /// Parse from string like "1.0.0"
    pub fn parse(s: &str) -> Result<Self, ContractError> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(ContractError::InvalidVersion(s.to_string()));
        }
        let major = parts[0]
            .parse()
            .map_err(|_| ContractError::InvalidVersion(s.to_string()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| ContractError::InvalidVersion(s.to_string()))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| ContractError::InvalidVersion(s.to_string()))?;
        Ok(Self { major, minor, patch })
    }
}

impl std::fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Contract metadata stored in contract.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractMetadata {
    pub contract_version: String,
    pub domain_id: String,
    pub domain_name: String,
    pub description: String,
    pub author: String,
    pub created: String,
    pub updated: String,
}

/// Domain configuration stored in config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainConfig {
    pub ollama_model: String,
    #[serde(default = "default_persistence_interval")]
    pub session_persistence_interval: u64,
    #[serde(default = "default_meeting_turn_limit")]
    pub meeting_turn_limit: u32,
    #[serde(default = "default_prompt_template")]
    pub prompt_template: String,
    #[serde(default = "default_streaming")]
    pub streaming_enabled: bool,
    #[serde(default)]
    pub manipulation_detection: ManipulationDetectionConfig,
}

fn default_persistence_interval() -> u64 {
    60
}
fn default_meeting_turn_limit() -> u32 {
    10
}
fn default_prompt_template() -> String {
    "default".to_string()
}
fn default_streaming() -> bool {
    true
}

/// Manipulation detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManipulationDetectionConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_enabled")]
    pub log_attempts: bool,
    #[serde(default = "default_patterns")]
    pub patterns: Vec<String>,
}

fn default_enabled() -> bool {
    true
}

fn default_patterns() -> Vec<String> {
    vec![
        "you are now".to_string(),
        "ignore previous".to_string(),
        "act as".to_string(),
        "forget your role".to_string(),
    ]
}

impl Default for ManipulationDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_attempts: true,
            patterns: default_patterns(),
        }
    }
}

/// Expected directory structure for a Context Repository
pub struct ContextStructure;

impl ContextStructure {
    pub const CONTRACT_FILE: &'static str = "contract.json";
    pub const CONFIG_FILE: &'static str = "config.json";
    pub const PERSONAS_DIR: &'static str = "personas";
    pub const WORLD_MODEL_DIR: &'static str = "world-model";
    pub const SCENARIOS_DIR: &'static str = "scenarios";
    pub const TESTS_DIR: &'static str = "tests";

    pub const BUSINESS_FLOWS_FILE: &'static str = "business-flows.json";
    pub const RULES_FILE: &'static str = "rules.json";
    pub const PROBLEMS_FILE: &'static str = "problems.json";
    pub const CONSTRAINTS_FILE: &'static str = "constraints.json";

    /// Returns all required top-level files
    pub fn required_files() -> Vec<&'static str> {
        vec![Self::CONTRACT_FILE, Self::CONFIG_FILE]
    }

    /// Returns all required directories
    pub fn required_dirs() -> Vec<&'static str> {
        vec![
            Self::PERSONAS_DIR,
            Self::WORLD_MODEL_DIR,
            Self::SCENARIOS_DIR,
        ]
    }

    /// Returns all required world model files
    pub fn required_world_model_files() -> Vec<&'static str> {
        vec![
            Self::BUSINESS_FLOWS_FILE,
            Self::RULES_FILE,
            Self::PROBLEMS_FILE,
            Self::CONSTRAINTS_FILE,
        ]
    }
}

/// Errors related to Context Contract operations
#[derive(Debug, Error)]
pub enum ContractError {
    #[error("Invalid contract version: {0}")]
    InvalidVersion(String),

    #[error("Contract version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },

    #[error("Missing required file: {0}")]
    MissingFile(String),

    #[error("Missing required directory: {0}")]
    MissingDirectory(String),

    #[error("Invalid JSON in {file}: {error}")]
    InvalidJson { file: String, error: String },

    #[error("Schema validation failed for {file}: {errors:?}")]
    SchemaValidation { file: String, errors: Vec<String> },

    #[error("Missing required field '{field}' in {file}")]
    MissingField { file: String, field: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of validating a Context Repository against the contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }

    pub fn with_error(error: ValidationError) -> Self {
        Self {
            valid: false,
            errors: vec![error],
            warnings: vec![],
        }
    }

    pub fn merge(&mut self, other: ValidationResult) {
        if !other.valid {
            self.valid = false;
        }
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

/// A specific validation error with location and details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub file: String,
    pub field: Option<String>,
    pub error_type: ValidationErrorType,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Types of validation errors
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationErrorType {
    MissingFile,
    MissingDirectory,
    InvalidJson,
    MissingField,
    InvalidValue,
    SchemaViolation,
    VersionMismatch,
}
