//! Quality assurance and manipulation detection models
//!
//! Defines structures for manipulation attempt tracking,
//! quality reports, and fidelity validation results.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A detected manipulation attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManipulationAttempt {
    pub attempt_id: String,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub input: String,
    pub detected_patterns: Vec<String>,
    pub severity: ManipulationSeverity,
    pub blocked: bool,
}

/// Severity levels for manipulation attempts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManipulationSeverity {
    Low,
    Medium,
    High,
}

/// Log of manipulation attempts for a session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManipulationLog {
    pub session_id: String,
    pub attempts: Vec<ManipulationAttempt>,
    pub summary: ManipulationSummary,
}

/// Summary statistics for manipulation attempts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManipulationSummary {
    pub total_attempts: u32,
    pub pattern_frequency: HashMap<String, u32>,
    pub severity_distribution: HashMap<String, u32>,
}

/// Report on response quality for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityReport {
    pub session_id: String,
    pub generated_at: DateTime<Utc>,
    pub total_consultations: u32,
    pub average_hallucination_score: f64,
    pub average_fidelity_score: f64,
    pub uncertainty_rate: f64,
    pub redirection_rate: f64,
    pub history_reference_rate: f64,
    pub per_persona_metrics: HashMap<String, PersonaQualityMetrics>,
}

/// Per-persona quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaQualityMetrics {
    pub persona_id: String,
    pub consultations: u32,
    pub average_response_time_ms: f64,
    pub average_response_length: f64,
    pub hallucination_score: f64,
    pub fidelity_score: f64,
    pub uncertainty_expressed_count: u32,
    pub boundary_violations: u32,
}

/// Result of a fidelity test run
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FidelityTestResult {
    pub test_id: String,
    pub persona_id: String,
    pub passed: bool,
    pub score: f64,
    pub details: Vec<FidelityTestDetail>,
}

/// Detail of a single fidelity test criterion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FidelityTestDetail {
    pub criterion: String,
    pub met: bool,
    pub explanation: String,
}

/// Report on persona behavioral consistency
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyReport {
    pub session_id: String,
    pub persona_id: String,
    pub generated_at: DateTime<Utc>,
    pub consultations_analyzed: u32,
    pub contradictions: Vec<Contradiction>,
    pub priority_drift_score: f64,
    pub overall_consistency_score: f64,
}

/// A detected contradiction in persona behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contradiction {
    pub consultation_a_id: String,
    pub consultation_b_id: String,
    pub issue: String,
    pub statement_a: String,
    pub statement_b: String,
    pub justified: bool,
}
