//! Scenario configuration models
//!
//! Defines learning scenarios with initial states, objectives,
//! triggering events, and success criteria.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A learning scenario configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioDefinition {
    /// Unique scenario identifier
    pub scenario_id: String,

    /// Human-readable scenario name
    pub name: String,

    /// Scenario description
    pub description: String,

    /// What students should learn from this scenario
    pub learning_objectives: Vec<String>,

    /// Initial state when scenario starts
    pub initial_state: HashMap<String, serde_json::Value>,

    /// Which domain experts are available in this scenario
    pub available_experts: Vec<String>,

    /// Events that trigger state changes
    #[serde(default)]
    pub triggering_events: Vec<TriggeringEvent>,

    /// Criteria for evaluating student success
    pub success_criteria: Vec<String>,

    /// Challenges students will face
    #[serde(default)]
    pub challenges: Vec<String>,
}

/// An event that triggers a state change in the scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggeringEvent {
    /// Event identifier
    pub event: String,

    /// Condition that triggers this event
    pub condition: String,

    /// Description of the state change
    pub state_change: String,
}

impl ScenarioDefinition {
    /// Validate the scenario definition
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.scenario_id.is_empty() {
            errors.push("scenarioId is required".to_string());
        }
        if self.name.is_empty() {
            errors.push("name is required".to_string());
        }
        if self.learning_objectives.is_empty() {
            errors.push("learningObjectives must contain at least one item".to_string());
        }
        if self.available_experts.is_empty() {
            errors.push("availableExperts must contain at least one expert".to_string());
        }
        if self.success_criteria.is_empty() {
            errors.push("successCriteria must contain at least one criterion".to_string());
        }

        errors
    }
}
