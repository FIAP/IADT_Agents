//! Context Repository loader
//!
//! Loads and validates a Context Repository from the filesystem,
//! checking against the Context Contract specification.

use std::path::{Path, PathBuf};

use crate::contract::*;
use crate::models::persona::{PersonaDefinition, PersonaFidelityTest};
use crate::models::scenario::ScenarioDefinition;
use crate::models::world_model::{BusinessFlows, DomainConstraints, DomainProblems, DomainRules, WorldModel};

/// A fully loaded and validated Context Repository
#[derive(Debug, Clone)]
pub struct LoadedContext {
    pub root_path: PathBuf,
    pub contract: ContractMetadata,
    pub config: DomainConfig,
    pub personas: Vec<PersonaDefinition>,
    pub world_model: WorldModel,
    pub scenarios: Vec<ScenarioDefinition>,
    pub fidelity_tests: Vec<PersonaFidelityTest>,
}

/// Loads a Context Repository from a directory path
pub fn load_context_repository(path: &Path) -> Result<LoadedContext, ContractError> {
    // Validate directory structure
    validate_directory_structure(path)?;

    // Load contract metadata
    let contract_path = path.join(ContextStructure::CONTRACT_FILE);
    let contract: ContractMetadata = load_json_file(&contract_path)?;

    // Validate contract version
    let _version = SemanticVersion::parse(&contract.contract_version)?;

    // Load domain config
    let config_path = path.join(ContextStructure::CONFIG_FILE);
    let config: DomainConfig = load_json_file(&config_path)?;

    // Load personas
    let personas_dir = path.join(ContextStructure::PERSONAS_DIR);
    let personas = load_personas(&personas_dir)?;

    // Load world model
    let world_model_dir = path.join(ContextStructure::WORLD_MODEL_DIR);
    let world_model = load_world_model(&world_model_dir)?;

    // Load scenarios
    let scenarios_dir = path.join(ContextStructure::SCENARIOS_DIR);
    let scenarios = load_scenarios(&scenarios_dir)?;

    // Load fidelity tests (optional)
    let tests_dir = path.join(ContextStructure::TESTS_DIR);
    let fidelity_tests = if tests_dir.exists() {
        load_fidelity_tests(&tests_dir)?
    } else {
        Vec::new()
    };

    Ok(LoadedContext {
        root_path: path.to_path_buf(),
        contract,
        config,
        personas,
        world_model,
        scenarios,
        fidelity_tests,
    })
}

/// Validate the directory structure of a Context Repository
fn validate_directory_structure(path: &Path) -> Result<(), ContractError> {
    if !path.exists() {
        return Err(ContractError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Context repository path does not exist: {}", path.display()),
        )));
    }

    // Check required files
    for file in ContextStructure::required_files() {
        let file_path = path.join(file);
        if !file_path.exists() {
            return Err(ContractError::MissingFile(file.to_string()));
        }
    }

    // Check required directories
    for dir in ContextStructure::required_dirs() {
        let dir_path = path.join(dir);
        if !dir_path.exists() {
            return Err(ContractError::MissingDirectory(dir.to_string()));
        }
    }

    // Check required world model files
    let wm_dir = path.join(ContextStructure::WORLD_MODEL_DIR);
    for file in ContextStructure::required_world_model_files() {
        let file_path = wm_dir.join(file);
        if !file_path.exists() {
            return Err(ContractError::MissingFile(format!(
                "{}/{}",
                ContextStructure::WORLD_MODEL_DIR,
                file
            )));
        }
    }

    Ok(())
}

/// Load and deserialize a JSON file
fn load_json_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ContractError> {
    let content = std::fs::read_to_string(path).map_err(ContractError::Io)?;
    serde_json::from_str(&content).map_err(|e| ContractError::InvalidJson {
        file: path.display().to_string(),
        error: e.to_string(),
    })
}

/// Load all persona definitions from the personas directory
fn load_personas(dir: &Path) -> Result<Vec<PersonaDefinition>, ContractError> {
    let mut personas = Vec::new();

    for entry in std::fs::read_dir(dir).map_err(ContractError::Io)? {
        let entry = entry.map_err(ContractError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let persona: PersonaDefinition = load_json_file(&path)?;

            // Validate persona fields
            let errors = persona.validate();
            if !errors.is_empty() {
                return Err(ContractError::SchemaValidation {
                    file: path.display().to_string(),
                    errors,
                });
            }

            personas.push(persona);
        }
    }

    if personas.is_empty() {
        return Err(ContractError::MissingFile(
            "No persona JSON files found in personas directory".to_string(),
        ));
    }

    Ok(personas)
}

/// Load world model from the world-model directory
fn load_world_model(dir: &Path) -> Result<WorldModel, ContractError> {
    let business_flows: BusinessFlows =
        load_json_file(&dir.join(ContextStructure::BUSINESS_FLOWS_FILE))?;
    let rules: DomainRules = load_json_file(&dir.join(ContextStructure::RULES_FILE))?;
    let problems: DomainProblems = load_json_file(&dir.join(ContextStructure::PROBLEMS_FILE))?;
    let constraints: DomainConstraints =
        load_json_file(&dir.join(ContextStructure::CONSTRAINTS_FILE))?;

    let world_model = WorldModel {
        business_flows,
        rules,
        problems,
        constraints,
    };

    // Validate world model
    let errors = world_model.validate();
    if !errors.is_empty() {
        return Err(ContractError::SchemaValidation {
            file: "world-model/".to_string(),
            errors,
        });
    }

    Ok(world_model)
}

/// Load all scenario definitions from the scenarios directory
fn load_scenarios(dir: &Path) -> Result<Vec<ScenarioDefinition>, ContractError> {
    let mut scenarios = Vec::new();

    for entry in std::fs::read_dir(dir).map_err(ContractError::Io)? {
        let entry = entry.map_err(ContractError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let scenario: ScenarioDefinition = load_json_file(&path)?;

            let errors = scenario.validate();
            if !errors.is_empty() {
                return Err(ContractError::SchemaValidation {
                    file: path.display().to_string(),
                    errors,
                });
            }

            scenarios.push(scenario);
        }
    }

    if scenarios.is_empty() {
        return Err(ContractError::MissingFile(
            "No scenario JSON files found in scenarios directory".to_string(),
        ));
    }

    Ok(scenarios)
}

/// Load persona fidelity tests from the tests directory
fn load_fidelity_tests(dir: &Path) -> Result<Vec<PersonaFidelityTest>, ContractError> {
    let mut tests = Vec::new();

    if !dir.exists() {
        return Ok(tests);
    }

    for entry in std::fs::read_dir(dir).map_err(ContractError::Io)? {
        let entry = entry.map_err(ContractError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let test: PersonaFidelityTest = load_json_file(&path)?;
            tests.push(test);
        }
    }

    Ok(tests)
}

/// Validate a Context Repository and return a detailed validation result
pub fn validate_context_repository(path: &Path) -> ValidationResult {
    let mut result = ValidationResult::success();

    // Check path exists
    if !path.exists() {
        result.valid = false;
        result.errors.push(ValidationError {
            file: path.display().to_string(),
            field: None,
            error_type: ValidationErrorType::MissingDirectory,
            message: "Context repository path does not exist".to_string(),
            suggestion: Some("Verify the path points to a valid context repository".to_string()),
        });
        return result;
    }

    // Check required files
    for file in ContextStructure::required_files() {
        let file_path = path.join(file);
        if !file_path.exists() {
            result.valid = false;
            result.errors.push(ValidationError {
                file: file.to_string(),
                field: None,
                error_type: ValidationErrorType::MissingFile,
                message: format!("Required file '{}' is missing", file),
                suggestion: Some(format!("Create '{}' in the context repository root", file)),
            });
        }
    }

    // Check required directories
    for dir in ContextStructure::required_dirs() {
        let dir_path = path.join(dir);
        if !dir_path.exists() {
            result.valid = false;
            result.errors.push(ValidationError {
                file: dir.to_string(),
                field: None,
                error_type: ValidationErrorType::MissingDirectory,
                message: format!("Required directory '{}' is missing", dir),
                suggestion: Some(format!(
                    "Create '{}/' directory in the context repository",
                    dir
                )),
            });
        }
    }

    // If basic structure is invalid, return early
    if !result.valid {
        return result;
    }

    // Validate contract.json
    let contract_result = validate_json_file::<ContractMetadata>(
        &path.join(ContextStructure::CONTRACT_FILE),
        ContextStructure::CONTRACT_FILE,
    );
    result.merge(contract_result);

    // Validate config.json
    let config_result = validate_json_file::<DomainConfig>(
        &path.join(ContextStructure::CONFIG_FILE),
        ContextStructure::CONFIG_FILE,
    );
    result.merge(config_result);

    // Validate world model files
    let wm_dir = path.join(ContextStructure::WORLD_MODEL_DIR);
    for file in ContextStructure::required_world_model_files() {
        let file_path = wm_dir.join(file);
        if !file_path.exists() {
            result.valid = false;
            result.errors.push(ValidationError {
                file: format!("{}/{}", ContextStructure::WORLD_MODEL_DIR, file),
                field: None,
                error_type: ValidationErrorType::MissingFile,
                message: format!("Required world model file '{}' is missing", file),
                suggestion: Some(format!(
                    "Create '{}' in the '{}/' directory",
                    file,
                    ContextStructure::WORLD_MODEL_DIR
                )),
            });
        }
    }

    // Validate persona files exist and parse
    let personas_dir = path.join(ContextStructure::PERSONAS_DIR);
    if personas_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&personas_dir) {
            let json_count = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|s| s.to_str())
                        == Some("json")
                })
                .count();
            if json_count == 0 {
                result.valid = false;
                result.errors.push(ValidationError {
                    file: ContextStructure::PERSONAS_DIR.to_string(),
                    field: None,
                    error_type: ValidationErrorType::MissingFile,
                    message: "No persona definition files found".to_string(),
                    suggestion: Some(
                        "Add at least one .json persona file to the personas/ directory"
                            .to_string(),
                    ),
                });
            }
        }
    }

    result
}

/// Validate a single JSON file can be deserialized
fn validate_json_file<T: serde::de::DeserializeOwned>(
    path: &Path,
    name: &str,
) -> ValidationResult {
    let mut result = ValidationResult::success();

    match std::fs::read_to_string(path) {
        Ok(content) => {
            if let Err(e) = serde_json::from_str::<T>(&content) {
                result.valid = false;
                result.errors.push(ValidationError {
                    file: name.to_string(),
                    field: None,
                    error_type: ValidationErrorType::InvalidJson,
                    message: format!("JSON parsing error: {}", e),
                    suggestion: Some("Fix the JSON syntax error".to_string()),
                });
            }
        }
        Err(e) => {
            result.valid = false;
            result.errors.push(ValidationError {
                file: name.to_string(),
                field: None,
                error_type: ValidationErrorType::MissingFile,
                message: format!("Cannot read file: {}", e),
                suggestion: Some("Verify the file exists and has correct permissions".to_string()),
            });
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn repo_path() -> std::path::PathBuf {
        let candidates = [
            Path::new("../auto-repair-shop"),
            Path::new("auto-repair-shop"),
        ];
        for candidate in &candidates {
            if candidate.exists() {
                return candidate.to_path_buf();
            }
        }
        Path::new("../auto-repair-shop").to_path_buf()
    }

    #[test]
    fn test_load_auto_repair_shop_context() {
        let path = repo_path();
        if !path.exists() {
            eprintln!("Skipping: {:?} not found", path);
            return;
        }
        let result = load_context_repository(&path);
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let ctx = result.unwrap();
        assert_eq!(ctx.contract.domain_id, "auto-repair-shop");
        assert_eq!(ctx.contract.contract_version, "1.0.0");
    }

    #[test]
    fn test_three_personas_loaded() {
        let path = repo_path();
        if !path.exists() { return; }
        let ctx = load_context_repository(&path).expect("Should load");
        assert_eq!(ctx.personas.len(), 3, "Expected 3 personas");
        let ids: Vec<&str> = ctx.personas.iter().map(|p| p.persona_id.as_str()).collect();
        assert!(ids.contains(&"mechanic") && ids.contains(&"attendant") && ids.contains(&"owner"));
    }

    #[test]
    fn test_three_scenarios_loaded() {
        let path = repo_path();
        if !path.exists() { return; }
        let ctx = load_context_repository(&path).expect("Should load");
        assert!(ctx.scenarios.len() >= 3, "Expected at least 3 scenarios");
    }

    #[test]
    fn test_world_model_complete() {
        let path = repo_path();
        if !path.exists() { return; }
        let ctx = load_context_repository(&path).expect("Should load");
        assert!(!ctx.world_model.business_flows.flows.is_empty());
        assert!(!ctx.world_model.rules.rules.is_empty());
        assert!(!ctx.world_model.problems.problems.is_empty());
        assert!(!ctx.world_model.constraints.constraints.is_empty());
    }

    #[test]
    fn test_all_personas_valid() {
        let path = repo_path();
        if !path.exists() { return; }
        let ctx = load_context_repository(&path).expect("Should load");
        for p in &ctx.personas {
            let errs = p.validate();
            assert!(errs.is_empty(), "Persona '{}' invalid: {:?}", p.persona_id, errs);
        }
    }

    #[test]
    fn test_fidelity_tests_loaded() {
        let path = repo_path();
        if !path.exists() { return; }
        let ctx = load_context_repository(&path).expect("Should load");
        assert_eq!(ctx.fidelity_tests.len(), 3, "Expected 3 fidelity test files");
    }
}
