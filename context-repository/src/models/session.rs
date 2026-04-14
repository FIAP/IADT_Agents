//! Session management models
//!
//! Defines session state, decision history, consultation records,
//! and meeting data structures for persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A complete session with all interaction history
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub session_id: String,
    pub context_repository_id: String,
    pub contract_version: String,
    pub scenario_id: String,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub state: SessionState,
    pub decision_history: Vec<Decision>,
    pub consultation_history: Vec<Consultation>,
    pub meeting_history: Vec<Meeting>,
    pub metrics: SessionMetrics,
}

impl Session {
    /// Create a new session
    pub fn new(context_id: &str, contract_version: &str, scenario_id: &str) -> Self {
        let now = Utc::now();
        Self {
            session_id: Uuid::new_v4().to_string(),
            context_repository_id: context_id.to_string(),
            contract_version: contract_version.to_string(),
            scenario_id: scenario_id.to_string(),
            created_at: now,
            last_accessed_at: now,
            state: SessionState::default(),
            decision_history: Vec::new(),
            consultation_history: Vec::new(),
            meeting_history: Vec::new(),
            metrics: SessionMetrics::default(),
        }
    }

    /// Record a new decision
    pub fn record_decision(&mut self, decision: Decision) {
        self.metrics.decisions_recorded += 1;
        self.decision_history.push(decision);
        self.last_accessed_at = Utc::now();
    }

    /// Record a new consultation
    pub fn record_consultation(&mut self, consultation: Consultation) {
        self.metrics.total_consultations += 1;
        let persona_count = self
            .metrics
            .consultations_per_persona
            .entry(consultation.persona_id.clone())
            .or_insert(0);
        *persona_count += 1;
        self.consultation_history.push(consultation);
        self.last_accessed_at = Utc::now();
    }

    /// Record a new meeting
    pub fn record_meeting(&mut self, meeting: Meeting) {
        self.metrics.meetings_held += 1;
        self.metrics.conflicts_identified += meeting.conflicts.len() as u32;
        self.meeting_history.push(meeting);
        self.last_accessed_at = Utc::now();
    }
}

/// Current session state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub current_scenario_state: HashMap<String, serde_json::Value>,
    pub active_constraints: Vec<String>,
    pub triggered_events: Vec<String>,
}

/// A student decision record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub decision_id: String,
    pub timestamp: DateTime<Utc>,
    pub description: String,
    #[serde(default)]
    pub reasoning: String,
    pub scenario_state: HashMap<String, serde_json::Value>,
    pub prior_consultations: Vec<String>,
    #[serde(default)]
    pub student_annotations: Vec<String>,
}

impl Decision {
    pub fn new(description: &str) -> Self {
        Self {
            decision_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            description: description.to_string(),
            reasoning: String::new(),
            scenario_state: HashMap::new(),
            prior_consultations: Vec::new(),
            student_annotations: Vec::new(),
        }
    }
}

/// A consultation with a domain expert
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Consultation {
    pub consultation_id: String,
    pub timestamp: DateTime<Utc>,
    pub persona_id: String,
    pub student_query: String,
    pub expert_response: String,
    pub response_time_ms: u64,
    pub quality_metrics: Option<QualityMetrics>,
    pub related_decisions: Vec<String>,
}

impl Consultation {
    pub fn new(persona_id: &str, query: &str, response: &str, response_time_ms: u64) -> Self {
        Self {
            consultation_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            persona_id: persona_id.to_string(),
            student_query: query.to_string(),
            expert_response: response.to_string(),
            response_time_ms,
            quality_metrics: None,
            related_decisions: Vec::new(),
        }
    }
}

/// A multi-persona meeting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub meeting_id: String,
    pub timestamp: DateTime<Utc>,
    pub participant_personas: Vec<String>,
    pub topic: String,
    pub turns: Vec<MeetingTurn>,
    pub conflicts: Vec<Conflict>,
    pub conclusion: Option<String>,
    pub duration_ms: u64,
}

impl Meeting {
    pub fn new(participants: Vec<String>, topic: &str) -> Self {
        Self {
            meeting_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            participant_personas: participants,
            topic: topic.to_string(),
            turns: Vec::new(),
            conflicts: Vec::new(),
            conclusion: None,
            duration_ms: 0,
        }
    }
}

/// A single turn in a meeting discussion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeetingTurn {
    pub turn_number: u32,
    pub persona_id: String,
    pub statement: String,
    pub responds_to: Option<u32>,
    pub timestamp: DateTime<Utc>,
}

/// A conflict detected during a meeting
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub conflict_id: String,
    pub personas: Vec<String>,
    pub issue: String,
    pub positions: HashMap<String, String>,
    pub resolved: bool,
    pub resolution: Option<String>,
}

/// Quality metrics for a consultation response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMetrics {
    pub response_time_ms: u64,
    pub response_length: usize,
    pub uncertainty_expressed: bool,
    pub knowledge_boundary_respected: bool,
    pub history_referenced: bool,
    pub redirected_appropriately: bool,
    pub hallucination_score: f64,
    pub fidelity_score: f64,
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            response_time_ms: 0,
            response_length: 0,
            uncertainty_expressed: false,
            knowledge_boundary_respected: true,
            history_referenced: false,
            redirected_appropriately: false,
            hallucination_score: 0.0,
            fidelity_score: 1.0,
        }
    }
}

/// Aggregated session metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetrics {
    pub total_consultations: u32,
    pub consultations_per_persona: HashMap<String, u32>,
    pub average_response_time_ms: f64,
    pub decisions_recorded: u32,
    pub meetings_held: u32,
    pub conflicts_identified: u32,
    pub manipulation_attempts: u32,
    pub quality_scores: QualityScores,
}

/// Aggregate quality scores
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityScores {
    pub average_hallucination: f64,
    pub average_fidelity: f64,
    pub uncertainty_rate: f64,
    pub redirection_rate: f64,
}
