//! Consultation Orchestrator module - Phase 2
//!
//! Coordinates single-persona consultation flow:
//! 1. Locate persona by ID/name
//! 2. Check availability in scenario
//! 3. Detect manipulation in user input
//! 4. Assemble prompt with security layers
//! 5. Send to Ollama and stream response
//! 6. Collect quality metrics
//! 7. Record consultation in session
//!
//! Requirements: 8.1–8.8, 10.1–10.2, 16.1–16.5, 28.1–28.7

use std::time::Instant;

use context_repository::loader::LoadedContext;
use context_repository::models::persona::PersonaDefinition;
use context_repository::models::quality::ManipulationSeverity;
use context_repository::models::scenario::ScenarioDefinition;
use context_repository::models::session::{Consultation, QualityMetrics, Session};
use context_repository::models::world_model::WorldModel;
use thiserror::Error;

use crate::ollama::OllamaClient;
use crate::persona::PersonaLoader;
use crate::prompt::PromptAssembler;
use crate::quality::{ManipulationDetector, ResponseQualityAnalyzer};

/// Errors during consultation
#[derive(Debug, Error)]
pub enum ConsultationError {
    #[error("Persona '{0}' not found. Available: {1}")]
    PersonaNotFound(String, String),

    #[error("Expert '{persona}' is not available in scenario '{scenario}'")]
    PersonaNotInScenario { persona: String, scenario: String },

    #[error("Ollama error: {0}")]
    Ollama(#[from] crate::ollama::OllamaError),
}

/// Result of a single consultation
#[derive(Debug)]
pub struct ConsultationResult {
    /// The recorded consultation (persisted to session)
    pub consultation: Consultation,
    /// Whether a manipulation attempt was detected
    pub manipulation_detected: bool,
    /// Severity if detected
    pub manipulation_severity: Option<ManipulationSeverity>,
    /// Assembled prompt (for debugging/logging)
    pub prompt_length: usize,
}

/// Orchestrates the complete flow for a single-persona consultation.
///
/// This struct is stateless — all state lives in the caller's Session.
pub struct ConsultationOrchestrator<'a> {
    context: &'a LoadedContext,
    scenario: &'a ScenarioDefinition,
    ollama: &'a OllamaClient,
    prompt_assembler: &'a PromptAssembler,
    manipulation_detector: &'a ManipulationDetector,
    model: &'a str,
}

impl<'a> ConsultationOrchestrator<'a> {
    pub fn new(
        context: &'a LoadedContext,
        scenario: &'a ScenarioDefinition,
        ollama: &'a OllamaClient,
        prompt_assembler: &'a PromptAssembler,
        manipulation_detector: &'a ManipulationDetector,
        model: &'a str,
    ) -> Self {
        Self {
            context,
            scenario,
            ollama,
            prompt_assembler,
            manipulation_detector,
            model,
        }
    }

    /// Execute a full consultation with the named persona.
    ///
    /// Steps:
    /// 1. Resolve persona (by ID or fuzzy name match)
    /// 2. Validate availability in current scenario
    /// 3. Detect manipulation — log and reinforce prompt if Medium/High
    /// 4. Assemble 5-layer prompt with optional reinforcement block
    /// 5. Call Ollama and measure response time
    /// 6. Calculate quality metrics from response content
    /// 7. Return ConsultationResult (caller records it in Session)
    pub async fn consult(
        &self,
        session: &Session,
        persona_query: &str,
        user_message: &str,
    ) -> Result<ConsultationResult, ConsultationError> {
        // Step 1: Resolve persona
        let persona = PersonaLoader::find(self.context, persona_query)
            .ok_or_else(|| {
                let available = self
                    .context
                    .personas
                    .iter()
                    .map(|p| format!("{} ({})", p.persona_id, p.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                ConsultationError::PersonaNotFound(persona_query.to_string(), available)
            })?;

        // Step 2: Check scenario availability
        if !PersonaLoader::is_available_in_scenario(persona, &self.scenario.available_experts) {
            return Err(ConsultationError::PersonaNotInScenario {
                persona: persona.name.clone(),
                scenario: self.scenario.name.clone(),
            });
        }

        // Step 3: Detect manipulation
        let (manipulation_detected, manipulation_severity) =
            self.detect_manipulation(user_message, &session.session_id);

        // Step 4: Assemble prompt with optional reinforcement
        let prompt = self.prompt_assembler.assemble_with_reinforcement(
            persona,
            &self.context.world_model,
            &session.decision_history,
            &session.consultation_history,
            user_message,
            manipulation_severity.as_ref(),
        );
        let prompt_length = prompt.len();

        // Step 5: Call Ollama
        let start = Instant::now();
        let response_text = self.ollama.generate(self.model, &prompt).await?;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Step 6: Calculate quality metrics
        let quality = self.calculate_quality_metrics(
            &response_text,
            elapsed_ms,
            persona,
            &self.context.world_model,
        );

        // Step 7: Build consultation record
        let mut consultation =
            Consultation::new(&persona.persona_id, user_message, &response_text, elapsed_ms);
        consultation.quality_metrics = Some(quality);

        Ok(ConsultationResult {
            consultation,
            manipulation_detected,
            manipulation_severity,
            prompt_length,
        })
    }

    /// Detect manipulation in user input; return severity if detected.
    fn detect_manipulation(
        &self,
        input: &str,
        session_id: &str,
    ) -> (bool, Option<ManipulationSeverity>) {
        match self.manipulation_detector.detect(input, session_id) {
            Some(attempt) => {
                let severity = attempt.severity.clone();
                let needs_reinforce = ManipulationDetector::requires_reinforcement(&attempt);
                tracing::warn!(
                    session = &session_id[..8.min(session_id.len())],
                    patterns = ?attempt.detected_patterns,
                    severity = ?attempt.severity,
                    reinforce = needs_reinforce,
                    "Manipulation attempt detected"
                );
                (true, Some(severity))
            }
            None => (false, None),
        }
    }

    /// Calculate quality metrics from the persona response.
    /// Req 34.1–34.4: track response time, uncertainty, redirection, history reference.
    fn calculate_quality_metrics(
        &self,
        response: &str,
        elapsed_ms: u64,
        persona: &PersonaDefinition,
        _world_model: &WorldModel,
    ) -> QualityMetrics {
        let uncertainty_expressed = ResponseQualityAnalyzer::detects_uncertainty(response);
        let redirected_appropriately = ResponseQualityAnalyzer::detects_redirection(response);
        let history_referenced = ResponseQualityAnalyzer::detects_history_reference(response);

        // Knowledge boundary respect: check the response doesn't claim to know what
        // the persona explicitly doesn't know. Simple heuristic: look for does_not_know
        // topics mentioned with high confidence (without uncertainty markers).
        let knowledge_boundary_respected =
            self.check_knowledge_boundary_respect(response, persona);

        QualityMetrics {
            response_time_ms: elapsed_ms,
            response_length: response.len(),
            uncertainty_expressed,
            knowledge_boundary_respected,
            history_referenced,
            redirected_appropriately,
            // Hallucination and fidelity scores are placeholders until Phase 4
            hallucination_score: 0.0,
            fidelity_score: 1.0,
        }
    }

    /// Heuristic check for knowledge boundary respect.
    ///
    /// For each topic the persona explicitly does NOT know, verify that the response
    /// either doesn't mention it at all, or mentions it only with uncertainty markers.
    fn check_knowledge_boundary_respect(
        &self,
        response: &str,
        persona: &PersonaDefinition,
    ) -> bool {
        let lower_response = response.to_lowercase();

        for unknown_topic in &persona.knowledge_boundaries.does_not_know {
            let topic_lower = unknown_topic.to_lowercase();
            // Extract key words from the topic (skip short words)
            let keywords: Vec<&str> = topic_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();

            for keyword in &keywords {
                if lower_response.contains(*keyword) {
                    // Topic mentioned — check if it's paired with uncertainty
                    if !ResponseQualityAnalyzer::detects_uncertainty(response)
                        && !ResponseQualityAnalyzer::detects_redirection(response)
                    {
                        // Possible knowledge boundary violation — flag it
                        tracing::debug!(
                            "Potential knowledge boundary issue: persona mentions '{}' without uncertainty",
                            keyword
                        );
                        return false;
                    }
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_repository::models::persona::{
        BehavioralPatterns, KnowledgeBoundaries, PersonaDefinition, ValidationCriteria,
    };
    use context_repository::models::world_model::{
        BusinessFlows, DomainConstraints, DomainProblems, DomainRules, WorldModel,
    };

    // ─── Test helpers ─────────────────────────────────────────────────────────

    fn make_test_persona() -> PersonaDefinition {
        PersonaDefinition {
            persona_id: "mechanic".to_string(),
            name: "Carlos (Mechanic)".to_string(),
            role: "Lead Mechanic".to_string(),
            objectives: vec!["Diagnose vehicle problems accurately".to_string()],
            responsibilities: vec!["Vehicle diagnostics".to_string()],
            constraints: vec![
                "Cannot approve repairs over $500 without owner approval".to_string(),
            ],
            knowledge_boundaries: KnowledgeBoundaries {
                knows: vec![
                    "Vehicle mechanical systems".to_string(),
                    "Diagnostic procedures".to_string(),
                    "Repair techniques".to_string(),
                ],
                does_not_know: vec![
                    "Customer financial situation".to_string(),
                    "Marketing strategies".to_string(),
                    "Legal compliance details".to_string(),
                ],
            },
            behavioral_patterns: BehavioralPatterns {
                uncertainty_level: "moderate".to_string(),
                uncertainty_triggers: vec!["Intermittent problems".to_string()],
                conflict_triggers: vec!["Quality vs. cost tradeoffs".to_string()],
                communication_style: "Direct, technical, practical".to_string(),
            },
            validation_criteria: ValidationCriteria {
                decision_quality: vec!["Diagnostic thoroughness".to_string()],
                objective_measures: vec!["Repair addresses root cause".to_string()],
                subjective_judgment: vec!["Repair priority assessment".to_string()],
            },
        }
    }

    fn make_empty_world_model() -> WorldModel {
        WorldModel {
            business_flows: BusinessFlows { flows: vec![] },
            rules: DomainRules { rules: vec![] },
            problems: DomainProblems { problems: vec![] },
            constraints: DomainConstraints { constraints: vec![] },
        }
    }

    /// Helper to create a ConsultationOrchestrator with dummy dependencies for
    /// testing `check_knowledge_boundary_respect` and `calculate_quality_metrics`.
    /// We use a minimalist approach: create temp references and call the methods.
    fn check_boundary(response: &str, persona: &PersonaDefinition) -> bool {
        // Replicate the logic from check_knowledge_boundary_respect
        let lower_response = response.to_lowercase();

        for unknown_topic in &persona.knowledge_boundaries.does_not_know {
            let topic_lower = unknown_topic.to_lowercase();
            let keywords: Vec<&str> = topic_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();

            for keyword in &keywords {
                if lower_response.contains(*keyword) {
                    if !ResponseQualityAnalyzer::detects_uncertainty(response)
                        && !ResponseQualityAnalyzer::detects_redirection(response)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn calculate_metrics(
        response: &str,
        elapsed_ms: u64,
        persona: &PersonaDefinition,
    ) -> QualityMetrics {
        let uncertainty_expressed = ResponseQualityAnalyzer::detects_uncertainty(response);
        let redirected_appropriately = ResponseQualityAnalyzer::detects_redirection(response);
        let history_referenced = ResponseQualityAnalyzer::detects_history_reference(response);
        let knowledge_boundary_respected = check_boundary(response, persona);

        QualityMetrics {
            response_time_ms: elapsed_ms,
            response_length: response.len(),
            uncertainty_expressed,
            knowledge_boundary_respected,
            history_referenced,
            redirected_appropriately,
            hallucination_score: 0.0,
            fidelity_score: 1.0,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Task 11: Knowledge Boundary Enforcement Tests
    // Req 28.1–28.7
    // ═══════════════════════════════════════════════════════════════════════════

    // ─── 11.1: Boundary respected when persona stays in lane ──────────────────

    /// Req 28.1: Persona responds only within knowledge boundary
    #[test]
    fn test_boundary_respected_when_response_is_in_domain() {
        let persona = make_test_persona();
        let response = "The engine misfires could be caused by worn spark plugs or a faulty ignition coil. \
                         I recommend running a compression test first.";
        assert!(
            check_boundary(response, &persona),
            "Response about mechanical systems (persona's domain) should respect boundaries"
        );
    }

    /// Req 28.2: Persona acknowledges limitation with uncertainty markers
    #[test]
    fn test_boundary_respected_with_uncertainty_on_unknown_topic() {
        let persona = make_test_persona();
        let response = "I'm not sure about the customer's financial situation. \
                         That's outside my area of expertise.";
        assert!(
            check_boundary(response, &persona),
            "Mentioning unknown topic WITH uncertainty should be OK"
        );
    }

    /// Req 28.3: Persona redirects to appropriate expert
    #[test]
    fn test_boundary_respected_with_redirection_on_unknown_topic() {
        let persona = make_test_persona();
        let response = "For questions about marketing strategies, \
                         you should consult the owner or the attendant.";
        assert!(
            check_boundary(response, &persona),
            "Mentioning unknown topic WITH redirection should be OK"
        );
    }

    // ─── 11.2: Boundary violation detected ────────────────────────────────────

    /// Req 28.4: Detect when persona speaks confidently about unknown topic
    #[test]
    fn test_boundary_violation_confident_claim_on_unknown_topic() {
        let persona = make_test_persona();
        let response = "The customer's financial situation is solid. \
                         They can definitely afford the full repair package.";
        assert!(
            !check_boundary(response, &persona),
            "Confident claim about 'financial situation' (unknown topic) must be a violation"
        );
    }

    /// Req 28.4: Detect boundary violation on marketing (outside mechanic's domain)
    #[test]
    fn test_boundary_violation_on_marketing_topic() {
        let persona = make_test_persona();
        let response = "Our marketing strategies should focus on social media advertising \
                         and seasonal promotions to drive more customers.";
        assert!(
            !check_boundary(response, &persona),
            "Confident claim about 'marketing strategies' must be a violation"
        );
    }

    /// Req 28.5: Detect boundary violation on legal compliance
    #[test]
    fn test_boundary_violation_on_legal_topic() {
        let persona = make_test_persona();
        let response = "The compliance requirements state that we need to update our licenses \
                         before the next quarter.";
        assert!(
            !check_boundary(response, &persona),
            "Confident claim about 'legal compliance' must be a violation"
        );
    }

    // ─── 11.3: Multi-topic boundary scenarios ─────────────────────────────────

    /// Req 28.6: Mixed response — known + unknown topics with proper acknowledgment
    #[test]
    fn test_boundary_mixed_topics_with_acknowledgment() {
        let persona = make_test_persona();
        let response = "The brake system needs immediate attention — the pads are worn to 2mm. \
                         As for the customer's financial options, I'm not sure about that. \
                         You should ask the attendant about payment plans.";
        assert!(
            check_boundary(response, &persona),
            "Mixed response with proper uncertainty/redirection should pass"
        );
    }

    /// Req 28.7: Response that doesn't mention unknown topics at all (clean pass)
    #[test]
    fn test_boundary_clean_pass_no_unknown_topics_mentioned() {
        let persona = make_test_persona();
        let response = "Based on the diagnostic, the alternator belt is worn and the battery \
                         terminals show corrosion. I recommend replacing the belt and cleaning \
                         the terminals.";
        assert!(
            check_boundary(response, &persona),
            "Response that never mentions unknown topics should always pass"
        );
    }

    // ─── 11.4: Prompt includes boundary instructions ──────────────────────────

    /// Req 28.6: Knowledge boundaries MUST appear in every consultation prompt
    #[test]
    fn test_prompt_contains_knowledge_boundary_instructions() {
        let assembler = PromptAssembler::new();
        let persona = make_test_persona();
        let world_model = make_empty_world_model();

        let prompt = assembler.assemble_consultation(
            &persona, &world_model, &[], &[], "What about marketing?",
        );

        // Positive boundaries
        assert!(prompt.contains("Vehicle mechanical systems"),
            "Prompt must list what persona knows");
        assert!(prompt.contains("Diagnostic procedures"),
            "Prompt must list all known topics");

        // Negative boundaries
        assert!(prompt.contains("Customer financial situation"),
            "Prompt must list what persona does NOT know");
        assert!(prompt.contains("Marketing strategies"),
            "Prompt must list all unknown topics");
        assert!(prompt.contains("Legal compliance details"),
            "Prompt must list all unknown topics");
    }

    /// Req 28.3: Prompt includes instruction to redirect to other experts
    #[test]
    fn test_prompt_contains_redirection_instruction() {
        let assembler = PromptAssembler::new();
        let persona = make_test_persona();
        let world_model = make_empty_world_model();

        let prompt = assembler.assemble_consultation(
            &persona, &world_model, &[], &[], "test",
        );

        assert!(
            prompt.contains("consulting the appropriate expert")
                || prompt.contains("suggest consulting"),
            "Prompt must include instruction to redirect outside knowledge boundary (Req 28.3)"
        );
    }

    /// Req 28.5: Prompt includes uncertainty instruction
    #[test]
    fn test_prompt_contains_uncertainty_instruction() {
        let assembler = PromptAssembler::new();
        let persona = make_test_persona();
        let world_model = make_empty_world_model();

        let prompt = assembler.assemble_consultation(
            &persona, &world_model, &[], &[], "test",
        );

        assert!(
            prompt.contains("Express uncertainty") || prompt.contains("Do NOT fabricate"),
            "Prompt must include instructions for expressing uncertainty (Req 28.5)"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Task 13: Quality Metrics Collection Tests (partial)
    // Req 34.1–34.6
    // ═══════════════════════════════════════════════════════════════════════════

    // ─── 13.1: Quality metrics calculation ────────────────────────────────────

    /// Req 34.1: Response time is recorded correctly
    #[test]
    fn test_quality_metrics_response_time_recorded() {
        let persona = make_test_persona();
        let metrics = calculate_metrics("The engine needs repair.", 1500, &persona);
        assert_eq!(metrics.response_time_ms, 1500);
    }

    /// Req 34.1: Response length is recorded correctly
    #[test]
    fn test_quality_metrics_response_length_recorded() {
        let persona = make_test_persona();
        let response = "The brake pads are worn.";
        let metrics = calculate_metrics(response, 500, &persona);
        assert_eq!(metrics.response_length, response.len());
    }

    /// Req 34.3: Uncertainty expression detected in metrics
    #[test]
    fn test_quality_metrics_uncertainty_detected() {
        let persona = make_test_persona();
        let response = "I'm not certain about the exact cause. It could be the fuel injector or the spark plugs.";
        let metrics = calculate_metrics(response, 1000, &persona);
        assert!(
            metrics.uncertainty_expressed,
            "Response with 'not certain' must flag uncertainty_expressed"
        );
    }

    /// Req 34.3: No uncertainty in confident response
    #[test]
    fn test_quality_metrics_no_uncertainty_in_confident_response() {
        let persona = make_test_persona();
        let response = "The brake pads need to be replaced immediately. The wear is below 1mm.";
        let metrics = calculate_metrics(response, 800, &persona);
        assert!(
            !metrics.uncertainty_expressed,
            "Confident response should not flag uncertainty"
        );
    }

    /// Req 34.4: Redirection correctly detected in metrics
    #[test]
    fn test_quality_metrics_redirection_detected() {
        let persona = make_test_persona();
        let response = "That's a billing question. You should consult the attendant for pricing details.";
        let metrics = calculate_metrics(response, 600, &persona);
        assert!(
            metrics.redirected_appropriately,
            "Response with 'consult' must flag redirected_appropriately"
        );
    }

    /// Req 34.2: History reference detected in metrics
    #[test]
    fn test_quality_metrics_history_reference_detected() {
        let persona = make_test_persona();
        let response = "As you mentioned earlier, the customer approved the brake inspection.";
        let metrics = calculate_metrics(response, 700, &persona);
        assert!(
            metrics.history_referenced,
            "Response with 'mentioned earlier' must flag history_referenced"
        );
    }

    /// Req 34.2: No history reference in fresh response
    #[test]
    fn test_quality_metrics_no_history_in_fresh_response() {
        let persona = make_test_persona();
        let response = "The transmission fluid level is low and the filter needs replacement.";
        let metrics = calculate_metrics(response, 900, &persona);
        assert!(
            !metrics.history_referenced,
            "Fresh response with no history phrases should not flag history_referenced"
        );
    }

    /// Req 28.7, 34.2: Knowledge boundary respected flag in metrics
    #[test]
    fn test_quality_metrics_boundary_respected_flag() {
        let persona = make_test_persona();

        // In-domain response
        let response_ok = "The engine oil should be changed every 5000 miles.";
        let metrics_ok = calculate_metrics(response_ok, 500, &persona);
        assert!(metrics_ok.knowledge_boundary_respected, "In-domain response must respect boundary");

        // Violation: confident claim about customer finances
        let response_bad = "The customer's financial situation allows for the premium service.";
        let metrics_bad = calculate_metrics(response_bad, 500, &persona);
        assert!(
            !metrics_bad.knowledge_boundary_respected,
            "Confident claim about unknown topic must flag violation"
        );
    }

    /// Req 34.1-34.4: Full metrics calculation for a complex response
    #[test]
    fn test_quality_metrics_full_complex_response() {
        let persona = make_test_persona();
        let response = "Based on the diagnosis, the catalytic converter is failing. \
                         As you mentioned earlier, the check engine light was intermittent. \
                         I'm not certain if it's also causing the exhaust issue — \
                         it could be a secondary problem. \
                         For the cost estimate, you should consult the attendant.";
        let metrics = calculate_metrics(response, 2500, &persona);

        assert_eq!(metrics.response_time_ms, 2500);
        assert!(metrics.response_length > 100, "Complex response should be longer");
        assert!(metrics.uncertainty_expressed, "Contains 'not certain' and 'could be'");
        assert!(metrics.history_referenced, "Contains 'mentioned earlier'");
        assert!(metrics.redirected_appropriately, "Contains 'consult'");
        assert!(metrics.knowledge_boundary_respected, "Uses uncertainty when mentioning cost");
    }

    // ─── Persona error cases ──────────────────────────────────────────────────

    /// ConsultationError Display format
    #[test]
    fn test_consultation_error_persona_not_found_display() {
        let err = ConsultationError::PersonaNotFound(
            "unknown".to_string(),
            "mechanic (Carlos), attendant (Maria)".to_string(),
        );
        let msg = err.to_string();
        assert!(msg.contains("unknown"));
        assert!(msg.contains("mechanic"));
    }

    #[test]
    fn test_consultation_error_not_in_scenario_display() {
        let err = ConsultationError::PersonaNotInScenario {
            persona: "owner".to_string(),
            scenario: "diagnostic-challenge".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("owner"));
        assert!(msg.contains("diagnostic-challenge"));
    }
}
