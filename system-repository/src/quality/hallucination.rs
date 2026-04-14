//! Hallucination Detection — Task 21
//!
//! Heuristic-based detection of fabricated or unsupported content
//! in Domain Expert responses.
//!
//! Detection strategies:
//! 1. Specificity without source — exact numbers, dates, names without justification
//! 2. Out-of-boundary knowledge claims — references outside knowledge boundaries
//! 3. Absolute certainty on uncertain topics — "definitely", "always" on uncertainty triggers
//! 4. Invented entities — referencing things not in the world model or session
//!
//! Requirements: 27.1–27.7

use context_repository::models::persona::PersonaDefinition;
use context_repository::models::world_model::WorldModel;

/// Result of hallucination analysis
#[derive(Debug, Clone)]
pub struct HallucinationAnalysis {
    /// Overall hallucination score. 0.0 = no hallucination, 1.0 = certain hallucination
    pub score: f64,
    /// Individual indicators found in the response
    pub indicators: Vec<HallucinationIndicator>,
}

/// A single hallucination indicator
#[derive(Debug, Clone)]
pub struct HallucinationIndicator {
    /// Type of indicator
    pub kind: HallucinationKind,
    /// Evidence found in the response
    pub evidence: String,
    /// Contribution to the overall score (0.0–1.0)
    pub weight: f64,
}

/// Types of hallucination indicators
#[derive(Debug, Clone, PartialEq)]
pub enum HallucinationKind {
    /// Response contains specific numbers/data without sourcing
    UnsourcedSpecificity,
    /// Response asserts knowledge outside persona's boundaries
    OutOfBoundaryKnowledge,
    /// Response uses absolute certainty on an uncertainty trigger topic
    AbsoluteCertaintyOnUncertainTopic,
    /// Response references entities not in the world model
    InventedEntity,
}

/// Detects potential hallucination in Domain Expert responses.
///
/// Uses heuristic analysis — not ML-based. False positive rate is expected
/// to be non-zero, so scores should be interpreted as risk indicators, not
/// definitive proof of hallucination.
pub struct HallucinationDetector;

impl HallucinationDetector {
    /// Analyze a response for hallucination indicators.
    ///
    /// Returns a score from 0.0 (no indicators) to 1.0 (high risk).
    pub fn analyze(
        response: &str,
        persona: &PersonaDefinition,
        world_model: &WorldModel,
    ) -> HallucinationAnalysis {
        let mut indicators = Vec::new();

        // Strategy 1: Unsourced specificity (exact numbers, percentages, dates)
        Self::detect_unsourced_specificity(response, &mut indicators);

        // Strategy 2: Out-of-boundary knowledge
        Self::detect_out_of_boundary(response, persona, &mut indicators);

        // Strategy 3: Absolute certainty on uncertain topics
        Self::detect_false_certainty(response, persona, &mut indicators);

        // Strategy 4: Invented entities
        Self::detect_invented_entities(response, world_model, &mut indicators);

        let score = Self::calculate_score(&indicators);

        HallucinationAnalysis { score, indicators }
    }

    /// Strategy 1: Detect specific numerical claims without justification.
    ///
    /// Exact prices, percentages, and dates are risky in responses since
    /// they suggest the LLM is inventing specifics.
    fn detect_unsourced_specificity(response: &str, indicators: &mut Vec<HallucinationIndicator>) {
        let specificity_patterns = [
            // Exact prices: "$1,234" or "$1234" or "R$1.234"
            (r"\$\d[\d,]{2,}", "exact dollar amount"),
            (r"R\$\s*\d[\d.]{2,}", "exact real amount"),
            // Exact percentages: "87.3%"
            (r"\d{2,3}\.\d+%", "precise percentage"),
            // Exact dates: "January 15, 2025" or "15/01/2025"
            (r"\d{1,2}/\d{1,2}/\d{4}", "exact date"),
            // Exact part numbers (fabricated specifics)
            (r"part\s*(?:number|#|no\.?)\s*\w{4,}", "specific part number"),
        ];

        for (pattern, description) in &specificity_patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(m) = re.find(response) {
                    indicators.push(HallucinationIndicator {
                        kind: HallucinationKind::UnsourcedSpecificity,
                        evidence: format!("{}: '{}'", description, m.as_str()),
                        weight: 0.15,
                    });
                }
            }
        }
    }

    /// Strategy 2: Detect confident claims about things the persona doesn't know.
    ///
    /// If the response mentions topics from `does_not_know` WITHOUT uncertainty
    /// markers, it's likely hallucination.
    fn detect_out_of_boundary(
        response: &str,
        persona: &PersonaDefinition,
        indicators: &mut Vec<HallucinationIndicator>,
    ) {
        let lower_response = response.to_lowercase();

        let uncertainty_markers = [
            "i'm not sure", "i don't know", "not certain",
            "outside my", "beyond my", "you should ask",
            "consult", "não tenho certeza", "não sei",
            "fora da minha", "consulte",
        ];

        let has_uncertainty = uncertainty_markers
            .iter()
            .any(|m| lower_response.contains(m));

        for unknown_topic in &persona.knowledge_boundaries.does_not_know {
            let topic_lower = unknown_topic.to_lowercase();
            let keywords: Vec<&str> = topic_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();

            for keyword in &keywords {
                if lower_response.contains(*keyword) && !has_uncertainty {
                    indicators.push(HallucinationIndicator {
                        kind: HallucinationKind::OutOfBoundaryKnowledge,
                        evidence: format!(
                            "Confident claim about '{}' (not in knowledge boundary)",
                            unknown_topic
                        ),
                        weight: 0.3,
                    });
                    break; // One indicator per topic
                }
            }
        }
    }

    /// Strategy 3: Detect absolute certainty on topics that should trigger uncertainty.
    ///
    /// When the response topic matches an uncertainty trigger but uses
    /// absolute language, it's likely the LLM is over-confident.
    fn detect_false_certainty(
        response: &str,
        persona: &PersonaDefinition,
        indicators: &mut Vec<HallucinationIndicator>,
    ) {
        let lower_response = response.to_lowercase();

        let absolute_markers = [
            "definitely", "absolutely", "guaranteed", "certainly",
            "100%", "always will", "never fails",
            "com certeza", "definitivamente", "garantido",
            "sempre vai", "nunca falha",
        ];

        let has_absolute = absolute_markers
            .iter()
            .any(|m| lower_response.contains(m));

        if !has_absolute {
            return;
        }

        for trigger in &persona.behavioral_patterns.uncertainty_triggers {
            let trigger_lower = trigger.to_lowercase();
            let keywords: Vec<&str> = trigger_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();

            let topic_mentioned = keywords
                .iter()
                .any(|kw| lower_response.contains(*kw));

            if topic_mentioned {
                indicators.push(HallucinationIndicator {
                    kind: HallucinationKind::AbsoluteCertaintyOnUncertainTopic,
                    evidence: format!(
                        "Absolute language used on uncertainty trigger topic: '{}'",
                        trigger
                    ),
                    weight: 0.25,
                });
            }
        }
    }

    /// Strategy 4: Detect references to entities not in the world model.
    ///
    /// If the response mentions specific employee names, company names,
    /// or tool names not defined in the world model, they're likely fabricated.
    fn detect_invented_entities(
        response: &str,
        world_model: &WorldModel,
        indicators: &mut Vec<HallucinationIndicator>,
    ) {
        let lower_response = response.to_lowercase();

        // Collect all known entity names from the world model
        let mut known_entities: Vec<String> = Vec::new();

        for flow in &world_model.business_flows.flows {
            known_entities.push(flow.name.to_lowercase());
        }
        for rule in &world_model.rules.rules {
            known_entities.push(rule.description.to_lowercase());
        }
        for problem in &world_model.problems.problems {
            known_entities.push(problem.name.to_lowercase());
        }
        for constraint in &world_model.constraints.constraints {
            known_entities.push(constraint.description.to_lowercase());
        }

        // Check for fabricated company/brand references
        // (common LLM hallucination: inventing specific vendor names)
        let fabrication_markers = [
            "according to our records",
            "our database shows",
            "the system indicates",
            "based on the report from",
            "according to the manual",
        ];

        for marker in &fabrication_markers {
            if lower_response.contains(marker) {
                // Check if any known entity could justify this claim
                let justified = known_entities
                    .iter()
                    .any(|entity| {
                        entity.contains("record") || entity.contains("database")
                            || entity.contains("system") || entity.contains("report")
                            || entity.contains("manual")
                    });

                if !justified {
                    indicators.push(HallucinationIndicator {
                        kind: HallucinationKind::InventedEntity,
                        evidence: format!("Reference to non-existent source: '{}'", marker),
                        weight: 0.2,
                    });
                }
            }
        }
    }

    /// Calculate overall hallucination score from indicators.
    /// Score is capped at 1.0 and uses weighted sum with diminishing returns.
    fn calculate_score(indicators: &[HallucinationIndicator]) -> f64 {
        if indicators.is_empty() {
            return 0.0;
        }

        let raw: f64 = indicators.iter().map(|i| i.weight).sum();
        // Diminishing returns: cap at 1.0
        (raw).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_repository::models::persona::{
        BehavioralPatterns, KnowledgeBoundaries, ValidationCriteria,
    };
    use context_repository::models::world_model::{
        BusinessFlows, DomainConstraints, DomainProblems, DomainRules,
    };

    fn make_persona() -> PersonaDefinition {
        PersonaDefinition {
            persona_id: "mechanic".to_string(),
            name: "Carlos".to_string(),
            role: "Lead Mechanic".to_string(),
            objectives: vec!["Diagnose problems".to_string()],
            responsibilities: vec!["Diagnostics".to_string()],
            constraints: vec!["Cannot approve > $500".to_string()],
            knowledge_boundaries: KnowledgeBoundaries {
                knows: vec!["Vehicle mechanical systems".to_string()],
                does_not_know: vec![
                    "Customer financial situation".to_string(),
                    "Insurance claim details".to_string(),
                ],
            },
            behavioral_patterns: BehavioralPatterns {
                uncertainty_level: "moderate".to_string(),
                uncertainty_triggers: vec![
                    "Intermittent problems".to_string(),
                    "Multiple possible causes".to_string(),
                ],
                conflict_triggers: vec!["Quality vs cost".to_string()],
                communication_style: "Direct".to_string(),
            },
            validation_criteria: ValidationCriteria {
                decision_quality: vec!["Thoroughness".to_string()],
                objective_measures: vec!["Root cause".to_string()],
                subjective_judgment: vec!["Priority".to_string()],
            },
        }
    }

    fn make_world_model() -> WorldModel {
        WorldModel {
            business_flows: BusinessFlows { flows: vec![] },
            rules: DomainRules { rules: vec![] },
            problems: DomainProblems { problems: vec![] },
            constraints: DomainConstraints { constraints: vec![] },
        }
    }

    // ─── Strategy 1: Unsourced specificity ─────────────────────────────────

    /// Req 27.1: Detect fabricated exact prices
    #[test]
    fn test_detects_fabricated_exact_prices() {
        let result = HallucinationDetector::analyze(
            "The repair will cost exactly $1,450 for parts and labor.",
            &make_persona(),
            &make_world_model(),
        );
        assert!(result.score > 0.0, "Should detect unsourced exact price");
        assert!(result.indicators.iter().any(|i| i.kind == HallucinationKind::UnsourcedSpecificity));
    }

    /// Req 27.1: Detect fabricated percentages
    #[test]
    fn test_detects_fabricated_percentages() {
        let result = HallucinationDetector::analyze(
            "There's a 87.5% chance the problem is the fuel pump.",
            &make_persona(),
            &make_world_model(),
        );
        assert!(result.score > 0.0, "Should detect precise percentage");
    }

    /// No false positive on simple responses
    #[test]
    fn test_no_specificity_in_normal_response() {
        let result = HallucinationDetector::analyze(
            "The brake pads are worn and need replacement.",
            &make_persona(),
            &make_world_model(),
        );
        let specificity_count = result.indicators.iter()
            .filter(|i| i.kind == HallucinationKind::UnsourcedSpecificity)
            .count();
        assert_eq!(specificity_count, 0, "Normal response should not flag specificity");
    }

    // ─── Strategy 2: Out-of-boundary knowledge ────────────────────────────

    /// Req 27.3: Confident claim about unknown topic
    #[test]
    fn test_detects_out_of_boundary_knowledge() {
        let result = HallucinationDetector::analyze(
            "The customer's financial situation is stable enough for the premium package.",
            &make_persona(),
            &make_world_model(),
        );
        assert!(
            result.indicators.iter().any(|i| i.kind == HallucinationKind::OutOfBoundaryKnowledge),
            "Should detect confident claim about 'financial situation'"
        );
    }

    /// Req 27.4: Mentioning unknown topic WITH uncertainty is NOT hallucination
    #[test]
    fn test_no_boundary_violation_with_uncertainty() {
        let result = HallucinationDetector::analyze(
            "I'm not sure about the customer's financial situation. You should ask the attendant.",
            &make_persona(),
            &make_world_model(),
        );
        let boundary_count = result.indicators.iter()
            .filter(|i| i.kind == HallucinationKind::OutOfBoundaryKnowledge)
            .count();
        assert_eq!(boundary_count, 0, "Uncertain reference to unknown topic should not trigger");
    }

    // ─── Strategy 3: False certainty on uncertain topics ──────────────────

    /// Req 27.2: Absolute certainty on intermittent problem (uncertainty trigger)
    #[test]
    fn test_detects_false_certainty_on_uncertain_topic() {
        let result = HallucinationDetector::analyze(
            "The intermittent stalling is definitely caused by the fuel pump.",
            &make_persona(),
            &make_world_model(),
        );
        assert!(
            result.indicators.iter().any(|i| i.kind == HallucinationKind::AbsoluteCertaintyOnUncertainTopic),
            "Should detect absolute certainty on an uncertainty trigger topic"
        );
    }

    /// No false positive: certainty on non-trigger topic is OK
    #[test]
    fn test_no_false_certainty_on_normal_topic() {
        let result = HallucinationDetector::analyze(
            "The brake pads definitely need replacement — they're at 1mm.",
            &make_persona(),
            &make_world_model(),
        );
        let certainty_count = result.indicators.iter()
            .filter(|i| i.kind == HallucinationKind::AbsoluteCertaintyOnUncertainTopic)
            .count();
        assert_eq!(certainty_count, 0, "Certainty on non-trigger topic should not flag");
    }

    // ─── Strategy 4: Invented entities ────────────────────────────────────

    /// Req 27.1: Referencing non-existent records
    #[test]
    fn test_detects_invented_records() {
        let result = HallucinationDetector::analyze(
            "According to our records, this vehicle was last serviced 6 months ago.",
            &make_persona(),
            &make_world_model(),
        );
        assert!(
            result.indicators.iter().any(|i| i.kind == HallucinationKind::InventedEntity),
            "Should detect reference to non-existent records"
        );
    }

    /// No invented entity when response doesn't reference sources
    #[test]
    fn test_no_invented_entity_in_opinion() {
        let result = HallucinationDetector::analyze(
            "I think the transmission fluid should be changed.",
            &make_persona(),
            &make_world_model(),
        );
        let entity_count = result.indicators.iter()
            .filter(|i| i.kind == HallucinationKind::InventedEntity)
            .count();
        assert_eq!(entity_count, 0, "Opinion without source reference should not flag");
    }

    // ─── Score calculation ────────────────────────────────────────────────

    /// Score is 0.0 for clean responses
    #[test]
    fn test_clean_response_score_zero() {
        let result = HallucinationDetector::analyze(
            "The engine oil needs to be changed. Standard procedure.",
            &make_persona(),
            &make_world_model(),
        );
        assert_eq!(result.score, 0.0, "Clean response should have 0.0 hallucination score");
        assert!(result.indicators.is_empty());
    }

    /// Score is capped at 1.0
    #[test]
    fn test_score_capped_at_one() {
        let result = HallucinationDetector::analyze(
            "According to our records, the customer's financial situation shows $1,234 available. \
             The insurance claim details indicate a 87.5% coverage rate. \
             The intermittent problem is definitely the fuel pump.",
            &make_persona(),
            &make_world_model(),
        );
        assert!(result.score <= 1.0, "Score must be capped at 1.0, got {}", result.score);
        assert!(result.score > 0.5, "Multiple indicators should produce high score");
    }
}
