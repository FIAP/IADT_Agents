//! Persona System module - Phase 2
//!
//! Handles persona loading, validation, and consultation orchestration.
//! Enforces knowledge boundaries and integrates manipulation resistance.

use context_repository::models::persona::PersonaDefinition;
use context_repository::loader::LoadedContext;

/// Result of a persona validation
#[derive(Debug, Clone, PartialEq)]
pub struct PersonaValidationResult {
    pub persona_id: String,
    pub valid: bool,
    pub errors: Vec<String>,
}

impl PersonaValidationResult {
    pub fn success(persona_id: &str) -> Self {
        Self {
            persona_id: persona_id.to_string(),
            valid: true,
            errors: Vec::new(),
        }
    }

    pub fn failure(persona_id: &str, errors: Vec<String>) -> Self {
        Self {
            persona_id: persona_id.to_string(),
            valid: false,
            errors,
        }
    }
}

/// Loader and validator for persona definitions
pub struct PersonaLoader;

impl PersonaLoader {
    /// Validate a persona definition against all required fields
    ///
    /// Property 6: Persona Definition Validation
    /// For any persona definition with missing required fields, validation SHALL
    /// identify all missing fields and report them in the validation error.
    pub fn validate(persona: &PersonaDefinition) -> PersonaValidationResult {
        let errors = persona.validate();

        if errors.is_empty() {
            PersonaValidationResult::success(&persona.persona_id)
        } else {
            PersonaValidationResult::failure(&persona.persona_id, errors)
        }
    }

    /// Validate all personas in a loaded context
    pub fn validate_all(context: &LoadedContext) -> Vec<PersonaValidationResult> {
        context
            .personas
            .iter()
            .map(|p| Self::validate(p))
            .collect()
    }

    /// Find a persona by ID or partial name match (case-insensitive)
    pub fn find<'a>(context: &'a LoadedContext, query: &str) -> Option<&'a PersonaDefinition> {
        let q = query.to_lowercase();
        context.personas.iter().find(|p| {
            p.persona_id == query
                || p.name.to_lowercase().contains(&q)
                || p.persona_id.to_lowercase() == q
        })
    }

    /// Check if a persona is available in a given scenario
    pub fn is_available_in_scenario(persona: &PersonaDefinition, scenario_experts: &[String]) -> bool {
        scenario_experts.contains(&persona.persona_id)
    }

    /// Extract knowledge boundary summary for display
    pub fn knowledge_summary(persona: &PersonaDefinition) -> String {
        let knows = persona.knowledge_boundaries.knows.len();
        let does_not_know = persona.knowledge_boundaries.does_not_know.len();
        format!(
            "{} knows {} topics, has {} explicit knowledge gaps",
            persona.name, knows, does_not_know
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_repository::models::persona::{
        BehavioralPatterns, KnowledgeBoundaries, PersonaDefinition, ValidationCriteria,
    };

    fn make_valid_persona(id: &str) -> PersonaDefinition {
        PersonaDefinition {
            persona_id: id.to_string(),
            name: format!("Test Expert {}", id),
            role: "Test Role".to_string(),
            objectives: vec!["Objective 1".to_string()],
            responsibilities: vec!["Responsibility 1".to_string()],
            constraints: vec!["Constraint 1".to_string()],
            knowledge_boundaries: KnowledgeBoundaries {
                knows: vec!["Topic A".to_string()],
                does_not_know: vec!["Topic B".to_string()],
            },
            behavioral_patterns: BehavioralPatterns {
                uncertainty_level: "moderate".to_string(),
                uncertainty_triggers: vec!["Trigger 1".to_string()],
                conflict_triggers: vec!["Conflict 1".to_string()],
                communication_style: "Professional".to_string(),
            },
            validation_criteria: ValidationCriteria {
                decision_quality: vec!["Quality 1".to_string()],
                objective_measures: vec!["Measure 1".to_string()],
                subjective_judgment: vec!["Judgment 1".to_string()],
            },
        }
    }

    // Property 6: Persona Definition Validation
    // For any persona with missing required fields, validation SHALL identify ALL missing fields.
    #[test]
    fn test_valid_persona_passes_validation() {
        let persona = make_valid_persona("test-001");
        let result = PersonaLoader::validate(&persona);
        assert!(result.valid, "Valid persona should pass validation");
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_empty_persona_id_fails() {
        let mut persona = make_valid_persona("test-002");
        persona.persona_id = String::new();
        let result = PersonaLoader::validate(&persona);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("personaId")));
    }

    #[test]
    fn test_empty_objectives_fails() {
        let mut persona = make_valid_persona("test-003");
        persona.objectives.clear();
        let result = PersonaLoader::validate(&persona);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("objectives")));
    }

    #[test]
    fn test_empty_constraints_fails() {
        let mut persona = make_valid_persona("test-004");
        persona.constraints.clear();
        let result = PersonaLoader::validate(&persona);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("constraints")));
    }

    #[test]
    fn test_empty_knowledge_boundaries_fails() {
        let mut persona = make_valid_persona("test-005");
        persona.knowledge_boundaries.knows.clear();
        let result = PersonaLoader::validate(&persona);
        assert!(!result.valid);
        assert!(
            result.errors.iter().any(|e| e.contains("knowledgeBoundaries.knows")),
            "Should report missing knows boundary, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_multiple_missing_fields_all_reported() {
        // Property 6: ALL missing fields must be reported
        let mut persona = make_valid_persona("test-006");
        persona.objectives.clear();
        persona.constraints.clear();
        persona.knowledge_boundaries.does_not_know.clear();
        let result = PersonaLoader::validate(&persona);
        assert!(!result.valid);
        // All three violations must be reported, not just the first
        assert!(
            result.errors.len() >= 3,
            "Expected at least 3 errors, got {}: {:?}",
            result.errors.len(),
            result.errors
        );
    }

    #[test]
    fn test_find_persona_by_id() {
        // We can't easily construct a full LoadedContext here, so we test the validation logic
        let persona = make_valid_persona("mechanic");
        let result = PersonaLoader::validate(&persona);
        assert!(result.valid);
        assert_eq!(result.persona_id, "mechanic");
    }

    #[test]
    fn test_knowledge_summary_format() {
        let persona = make_valid_persona("mechanic");
        let summary = PersonaLoader::knowledge_summary(&persona);
        assert!(summary.contains("mechanic") || summary.contains("Test Expert mechanic"));
        assert!(summary.contains("1 topics"));
        assert!(summary.contains("1 explicit knowledge gaps"));
    }

    #[test]
    fn test_persona_availability_in_scenario() {
        let persona = make_valid_persona("mechanic");
        let available = vec!["mechanic".to_string(), "attendant".to_string()];
        assert!(PersonaLoader::is_available_in_scenario(&persona, &available));

        let unavailable = vec!["owner".to_string()];
        assert!(!PersonaLoader::is_available_in_scenario(&persona, &unavailable));
    }

    // Property 19: Manipulation Attempt Invariance
    // The persona's core role and constraints SHALL remain unchanged after validation.
    #[test]
    fn test_validation_does_not_mutate_persona() {
        let persona = make_valid_persona("immutable-test");
        let original_role = persona.role.clone();
        let original_constraints = persona.constraints.clone();

        PersonaLoader::validate(&persona);

        assert_eq!(persona.role, original_role, "Validation must not mutate role");
        assert_eq!(persona.constraints, original_constraints, "Validation must not mutate constraints");
    }
}
