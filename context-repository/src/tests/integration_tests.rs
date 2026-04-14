//! Integration tests for loading the auto-repair-shop context repository

#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::loader::load_context_repository;

    fn repo_path() -> &'static Path {
        Path::new("../auto-repair-shop")
    }

    #[test]
    fn test_load_auto_repair_shop_context() {
        let path = repo_path();
        if !path.exists() {
            eprintln!("Skipping: auto-repair-shop directory not found at {:?}", path);
            return;
        }

        let result = load_context_repository(path);
        assert!(
            result.is_ok(),
            "Context repository failed to load: {:?}",
            result.err()
        );

        let context = result.unwrap();
        assert_eq!(context.contract.domain_id, "auto-repair-shop");
        assert_eq!(context.contract.contract_version, "1.0.0");
    }

    #[test]
    fn test_auto_repair_has_three_personas() {
        let path = repo_path();
        if !path.exists() { return; }

        let context = load_context_repository(path).expect("Should load");
        assert_eq!(context.personas.len(), 3, "Expected 3 personas (mechanic, attendant, owner)");

        let ids: Vec<&str> = context.personas.iter().map(|p| p.persona_id.as_str()).collect();
        assert!(ids.contains(&"mechanic"), "mechanic persona missing");
        assert!(ids.contains(&"attendant"), "attendant persona missing");
        assert!(ids.contains(&"owner"), "owner persona missing");
    }

    #[test]
    fn test_auto_repair_has_three_scenarios() {
        let path = repo_path();
        if !path.exists() { return; }

        let context = load_context_repository(path).expect("Should load");
        assert_eq!(context.scenarios.len(), 3, "Expected 3 scenarios");

        let ids: Vec<&str> = context.scenarios.iter().map(|s| s.scenario_id.as_str()).collect();
        assert!(ids.contains(&"diagnostic-challenge"));
        assert!(ids.contains(&"parts-delay"));
        assert!(ids.contains(&"cost-objection"));
    }

    #[test]
    fn test_world_model_complete() {
        let path = repo_path();
        if !path.exists() { return; }

        let context = load_context_repository(path).expect("Should load");
        let wm = &context.world_model;

        assert!(!wm.business_flows.flows.is_empty(), "business-flows must not be empty");
        assert!(!wm.rules.rules.is_empty(), "rules must not be empty");
        assert!(!wm.problems.problems.is_empty(), "problems must not be empty");
        assert!(!wm.constraints.constraints.is_empty(), "constraints must not be empty");
    }

    #[test]
    fn test_fidelity_tests_loaded() {
        let path = repo_path();
        if !path.exists() { return; }

        let context = load_context_repository(path).expect("Should load");
        assert_eq!(
            context.fidelity_tests.len(), 3,
            "Expected fidelity tests for all 3 personas"
        );
    }

    #[test]
    fn test_all_personas_pass_validation() {
        let path = repo_path();
        if !path.exists() { return; }

        let context = load_context_repository(path).expect("Should load");
        for persona in &context.personas {
            let errors = persona.validate();
            assert!(
                errors.is_empty(),
                "Persona '{}' failed validation: {:?}",
                persona.persona_id,
                errors
            );
        }
    }

    #[test]
    fn test_mechanic_knowledge_boundaries_non_empty() {
        let path = repo_path();
        if !path.exists() { return; }

        let context = load_context_repository(path).expect("Should load");
        let mechanic = context.personas.iter()
            .find(|p| p.persona_id == "mechanic")
            .expect("mechanic persona must exist");

        assert!(!mechanic.knowledge_boundaries.knows.is_empty());
        assert!(!mechanic.knowledge_boundaries.does_not_know.is_empty());
    }
}
