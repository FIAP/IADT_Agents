//! Persona definition model
//!
//! Defines the structure for Domain Expert personas including role,
//! objectives, constraints, knowledge boundaries, and behavioral patterns.

use serde::{Deserialize, Serialize};

/// Complete persona definition for a Domain Expert
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaDefinition {
    /// Unique identifier for this persona
    pub persona_id: String,

    /// Display name (e.g., "Carlos (Mechanic)")
    pub name: String,

    /// Role title (e.g., "Lead Mechanic")
    pub role: String,

    /// Strategic objectives this persona pursues
    pub objectives: Vec<String>,

    /// Functional responsibilities
    pub responsibilities: Vec<String>,

    /// Operational constraints limiting this persona's actions
    pub constraints: Vec<String>,

    /// Knowledge boundaries defining expertise limits
    pub knowledge_boundaries: KnowledgeBoundaries,

    /// Behavioral patterns for realistic simulation
    pub behavioral_patterns: BehavioralPatterns,

    /// Criteria for evaluating student decisions
    pub validation_criteria: ValidationCriteria,
}

/// Defines what the persona knows and doesn't know
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBoundaries {
    /// Topics within this persona's expertise
    pub knows: Vec<String>,

    /// Topics explicitly outside this persona's expertise
    pub does_not_know: Vec<String>,
}

/// Behavioral patterns for realistic simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BehavioralPatterns {
    /// Base uncertainty level: "low", "moderate", "high"
    pub uncertainty_level: String,

    /// Situations that trigger uncertainty expressions
    pub uncertainty_triggers: Vec<String>,

    /// Situations that trigger conflict with other personas
    pub conflict_triggers: Vec<String>,

    /// Communication style description
    pub communication_style: String,
}

/// Criteria for evaluating student decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCriteria {
    /// Aspects of decision quality this persona evaluates
    pub decision_quality: Vec<String>,

    /// Objective, measurable criteria
    pub objective_measures: Vec<String>,

    /// Subjective professional judgment criteria
    pub subjective_judgment: Vec<String>,
}

/// Persona fidelity test definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaFidelityTest {
    /// Persona this test is for
    pub persona_id: String,

    /// Individual test cases
    pub tests: Vec<FidelityTestCase>,
}

/// A single fidelity test case
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FidelityTestCase {
    /// Unique test identifier
    pub test_id: String,

    /// Human-readable test name
    pub name: String,

    /// The input/question to present to the persona
    pub input: String,

    /// Expected behavioral traits in the response
    pub expected_behavior: ExpectedBehavior,

    /// Criteria for validating the response
    pub validation_criteria: Vec<String>,
}

/// Expected behavioral traits for a fidelity test
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedBehavior {
    #[serde(default)]
    pub should_not_know: bool,
    #[serde(default)]
    pub should_redirect: bool,
    #[serde(default)]
    pub redirect_to: Option<String>,
    #[serde(default)]
    pub should_not_fabricate: bool,
    #[serde(default)]
    pub should_express_uncertainty: bool,
    #[serde(default)]
    pub should_list_possibilities: bool,
    #[serde(default)]
    pub should_recommend_diagnostic: bool,
    #[serde(default)]
    pub should_refer_to_owner: bool,
    #[serde(default)]
    pub should_not_approve: bool,
    #[serde(default)]
    pub should_explain_constraint: bool,
    #[serde(default)]
    pub should_maintain_role: bool,
    #[serde(default)]
    pub should_reject_override: bool,
    #[serde(default)]
    pub should_reaffirm_role: bool,
}

impl PersonaDefinition {
    /// Validate that all required fields are present and non-empty
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.persona_id.is_empty() {
            errors.push("personaId is required".to_string());
        }
        if self.name.is_empty() {
            errors.push("name is required".to_string());
        }
        if self.role.is_empty() {
            errors.push("role is required".to_string());
        }
        if self.objectives.is_empty() {
            errors.push("objectives must contain at least one item".to_string());
        }
        if self.responsibilities.is_empty() {
            errors.push("responsibilities must contain at least one item".to_string());
        }
        if self.constraints.is_empty() {
            errors.push("constraints must contain at least one item".to_string());
        }
        if self.knowledge_boundaries.knows.is_empty() {
            errors.push("knowledgeBoundaries.knows must contain at least one item".to_string());
        }
        if self.knowledge_boundaries.does_not_know.is_empty() {
            errors.push(
                "knowledgeBoundaries.doesNotKnow must contain at least one item".to_string(),
            );
        }
        if self.behavioral_patterns.uncertainty_triggers.is_empty() {
            errors.push(
                "behavioralPatterns.uncertaintyTriggers must contain at least one item".to_string(),
            );
        }
        if self.behavioral_patterns.conflict_triggers.is_empty() {
            errors.push(
                "behavioralPatterns.conflictTriggers must contain at least one item".to_string(),
            );
        }
        if self.behavioral_patterns.communication_style.is_empty() {
            errors.push("behavioralPatterns.communicationStyle is required".to_string());
        }
        if self.validation_criteria.decision_quality.is_empty() {
            errors.push(
                "validationCriteria.decisionQuality must contain at least one item".to_string(),
            );
        }

        errors
    }
}
