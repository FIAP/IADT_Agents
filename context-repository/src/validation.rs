//! Validation utilities for Context Repository
//!
//! Provides additional validation beyond basic structural checks,
//! including cross-referencing between personas, scenarios, and world model.

use crate::contract::{ValidationError, ValidationErrorType, ValidationResult};
use crate::loader::LoadedContext;

/// Perform deep validation of a loaded context repository
pub fn validate_loaded_context(context: &LoadedContext) -> ValidationResult {
    let mut result = ValidationResult::success();

    // Validate all scenario experts reference existing personas
    let persona_ids: Vec<&str> = context.personas.iter().map(|p| p.persona_id.as_str()).collect();

    for scenario in &context.scenarios {
        for expert in &scenario.available_experts {
            if !persona_ids.contains(&expert.as_str()) {
                result.valid = false;
                result.errors.push(ValidationError {
                    file: format!("scenarios/{}.json", scenario.scenario_id),
                    field: Some("availableExperts".to_string()),
                    error_type: ValidationErrorType::InvalidValue,
                    message: format!(
                        "Scenario '{}' references unknown expert '{}'",
                        scenario.scenario_id, expert
                    ),
                    suggestion: Some(format!(
                        "Add a persona definition for '{}' or remove it from availableExperts. Available personas: {:?}",
                        expert, persona_ids
                    )),
                });
            }
        }
    }

    // Validate business flow actors reference existing personas
    for flow in &context.world_model.business_flows.flows {
        for step in &flow.steps {
            if !persona_ids.contains(&step.actor.as_str()) {
                result.warnings.push(format!(
                    "Flow '{}' step {} references actor '{}' which is not a defined persona",
                    flow.flow_id, step.step, step.actor
                ));
            }
        }
    }

    // Validate fidelity tests reference existing personas
    for test in &context.fidelity_tests {
        if !persona_ids.contains(&test.persona_id.as_str()) {
            result.valid = false;
            result.errors.push(ValidationError {
                file: format!("tests/{}-tests.json", test.persona_id),
                field: Some("personaId".to_string()),
                error_type: ValidationErrorType::InvalidValue,
                message: format!(
                    "Fidelity test references unknown persona '{}'",
                    test.persona_id
                ),
                suggestion: Some(format!(
                    "Ensure persona '{}' is defined in the personas/ directory",
                    test.persona_id
                )),
            });
        }
    }

    // Validate problems reference existing roles
    for problem in &context.world_model.problems.problems {
        for role in &problem.affected_roles {
            if !persona_ids.contains(&role.as_str()) {
                result.warnings.push(format!(
                    "Problem '{}' affects role '{}' which is not a defined persona",
                    problem.problem_id, role
                ));
            }
        }
    }

    result
}
