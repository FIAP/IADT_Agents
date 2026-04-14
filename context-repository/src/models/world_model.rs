//! World Model definitions
//!
//! Defines business flows, rules, problems, and constraints
//! that constitute the complete domain context.

use serde::{Deserialize, Serialize};

/// Complete World Model containing all domain context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldModel {
    pub business_flows: BusinessFlows,
    pub rules: DomainRules,
    pub problems: DomainProblems,
    pub constraints: DomainConstraints,
}

/// Business flow definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessFlows {
    pub flows: Vec<BusinessFlow>,
}

/// A single business flow describing an operational sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BusinessFlow {
    pub flow_id: String,
    pub name: String,
    pub steps: Vec<FlowStep>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// A step within a business flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowStep {
    pub step: u32,
    pub actor: String,
    pub action: String,
    pub outputs: Vec<String>,
}

/// Domain rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainRules {
    pub rules: Vec<DomainRule>,
}

/// A single domain rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainRule {
    pub rule_id: String,
    pub description: String,
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub applies_to: Option<Vec<String>>,
}

/// Domain problems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainProblems {
    pub problems: Vec<DomainProblem>,
}

/// A recurring domain problem
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainProblem {
    pub problem_id: String,
    pub name: String,
    pub description: String,
    pub frequency: String,
    pub affected_roles: Vec<String>,
    pub impacts: Vec<String>,
    pub typical_responses: Vec<String>,
}

/// Domain constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainConstraints {
    pub constraints: Vec<DomainConstraint>,
}

/// A domain constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainConstraint {
    pub constraint_id: String,
    #[serde(rename = "type")]
    pub constraint_type: String,
    pub description: String,
    pub affects: Vec<String>,
    /// Additional constraint-specific parameters stored generically
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

impl WorldModel {
    /// Validate that all required sections are present and non-empty
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.business_flows.flows.is_empty() {
            errors.push("businessFlows must contain at least one flow".to_string());
        }
        if self.rules.rules.is_empty() {
            errors.push("rules must contain at least one rule".to_string());
        }
        if self.problems.problems.is_empty() {
            errors.push("problems must contain at least one problem".to_string());
        }
        if self.constraints.constraints.is_empty() {
            errors.push("constraints must contain at least one constraint".to_string());
        }

        // Validate each flow has steps
        for flow in &self.business_flows.flows {
            if flow.steps.is_empty() {
                errors.push(format!("Flow '{}' must contain at least one step", flow.flow_id));
            }
        }

        errors
    }
}
