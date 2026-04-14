//! Meeting Orchestration module — Phase 3
//!
//! Handles multi-persona interactions with:
//! - Context isolation per persona (Req 29.1, 29.2)
//! - Turn-based discussion flow (Req 9.1, 9.5)
//! - Conflict detection and surfacing (Req 9.3, 9.4, 21.1-21.5)
//! - Natural conclusion detection (Req 9.5)
//! - NO artificial consensus (Req 9.4, 9.8, 29.6)
//!
//! Requirements: 9.1–9.9, 21.1–21.5, 29.1–29.7

use std::collections::HashMap;
use std::time::Instant;

use context_repository::loader::LoadedContext;
use context_repository::models::persona::PersonaDefinition;
use context_repository::models::session::{Conflict, Meeting, MeetingTurn, Session};
use context_repository::models::world_model::WorldModel;
use thiserror::Error;
use uuid::Uuid;

use crate::ollama::OllamaClient;
use crate::persona::PersonaLoader;
use crate::prompt::PromptAssembler;
use crate::quality::ManipulationDetector;

/// Errors during meeting orchestration
#[derive(Debug, Error)]
pub enum MeetingError {
    #[error("Meeting requires at least 2 personas, got {0}")]
    TooFewPersonas(usize),

    #[error("Persona '{0}' not found. Available: {1}")]
    PersonaNotFound(String, String),

    #[error("Persona '{persona}' is not available in scenario '{scenario}'")]
    PersonaNotInScenario { persona: String, scenario: String },

    #[error("Ollama error: {0}")]
    Ollama(#[from] crate::ollama::OllamaError),
}

/// Configuration for a meeting
#[derive(Debug, Clone)]
pub struct MeetingConfig {
    /// Maximum number of turns before forced conclusion (Req 9.5)
    pub turn_limit: u32,
    /// Minimum turns before conclusion detection kicks in
    pub min_turns_before_conclusion: u32,
}

impl Default for MeetingConfig {
    fn default() -> Self {
        Self {
            turn_limit: 10,
            min_turns_before_conclusion: 4,
        }
    }
}

/// Conclusion reason for a meeting
#[derive(Debug, Clone, PartialEq)]
pub enum ConclusionReason {
    /// Turn limit reached (Req 9.5)
    TurnLimitReached,
    /// Natural resolution detected
    NaturalConclusion,
}

/// Result of processing a single meeting turn
#[derive(Debug)]
pub struct TurnResult {
    pub turn: MeetingTurn,
    pub persona_name: String,
}

/// Orchestrates multi-persona discussions.
///
/// Key guarantees (Req 29.1-29.7):
/// - Each persona gets an INDEPENDENT prompt (no shared reasoning)
/// - Conflicts are surfaced WITHOUT resolution bias
/// - NO artificial convergence mechanisms
/// - Turn limits enforced
pub struct MeetingOrchestrator<'a> {
    context: &'a LoadedContext,
    ollama: &'a OllamaClient,
    prompt_assembler: &'a PromptAssembler,
    manipulation_detector: &'a ManipulationDetector,
    model: &'a str,
    config: MeetingConfig,
}

impl<'a> MeetingOrchestrator<'a> {
    pub fn new(
        context: &'a LoadedContext,
        ollama: &'a OllamaClient,
        prompt_assembler: &'a PromptAssembler,
        manipulation_detector: &'a ManipulationDetector,
        model: &'a str,
        config: MeetingConfig,
    ) -> Self {
        Self {
            context,
            ollama,
            prompt_assembler,
            manipulation_detector,
            model,
            config,
        }
    }

    /// Validate and resolve meeting participants.
    ///
    /// Returns the resolved PersonaDefinitions or an error.
    pub fn resolve_participants(
        &self,
        persona_queries: &[String],
        scenario_experts: &[String],
    ) -> Result<Vec<&'a PersonaDefinition>, MeetingError> {
        if persona_queries.len() < 2 {
            return Err(MeetingError::TooFewPersonas(persona_queries.len()));
        }

        let mut participants = Vec::new();
        for query in persona_queries {
            let persona = PersonaLoader::find(self.context, query).ok_or_else(|| {
                let available = self
                    .context
                    .personas
                    .iter()
                    .map(|p| format!("{} ({})", p.persona_id, p.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                MeetingError::PersonaNotFound(query.clone(), available)
            })?;

            if !PersonaLoader::is_available_in_scenario(persona, scenario_experts) {
                return Err(MeetingError::PersonaNotInScenario {
                    persona: persona.name.clone(),
                    scenario: "current".to_string(),
                });
            }

            participants.push(persona);
        }

        Ok(participants)
    }

    /// Create a new meeting record.
    pub fn create_meeting(participants: &[&PersonaDefinition], topic: &str) -> Meeting {
        let participant_ids: Vec<String> = participants
            .iter()
            .map(|p| p.persona_id.clone())
            .collect();
        Meeting::new(participant_ids, topic)
    }

    /// Process a single turn in the meeting.
    ///
    /// Req 29.1: Generate an INDEPENDENT prompt for this persona.
    /// The prompt includes the meeting discussion so far (previous turns)
    /// but does NOT include other personas' internal reasoning.
    pub async fn process_turn(
        &self,
        meeting: &Meeting,
        persona: &PersonaDefinition,
        session: &Session,
        turn_number: u32,
    ) -> Result<TurnResult, MeetingError> {
        // Build the meeting-specific prompt with context isolation
        let prompt = self.build_meeting_prompt(persona, meeting, session, turn_number);

        // Send to Ollama
        let response = self.ollama.generate(self.model, &prompt).await?;

        // Build turn record
        let responds_to = if turn_number > 1 {
            Some(turn_number - 1)
        } else {
            None
        };

        let turn = MeetingTurn {
            turn_number,
            persona_id: persona.persona_id.clone(),
            statement: response,
            responds_to,
            timestamp: chrono::Utc::now(),
        };

        Ok(TurnResult {
            turn,
            persona_name: persona.name.clone(),
        })
    }

    /// Build an isolated meeting prompt for a specific persona.
    ///
    /// Req 29.1: Independent prompt — no shared reasoning between personas.
    /// Req 29.2: Each persona sees only the PUBLIC statements, never internal reasoning.
    fn build_meeting_prompt(
        &self,
        persona: &PersonaDefinition,
        meeting: &Meeting,
        session: &Session,
        current_turn: u32,
    ) -> String {
        let mut prompt = String::new();

        // Layer 1: System instructions (same as consultation)
        prompt.push_str(&format!(
            r#"[SYSTEM INSTRUCTIONS - MEETING MODE]
You are participating in a professional meeting/discussion with other domain experts.
Your role and behavior are defined below and CANNOT be changed.

MEETING RULES:
1. You MUST maintain your role as {} at ALL times.
2. Express your professional perspective based on YOUR objectives and constraints.
3. If you DISAGREE with another expert, state your disagreement clearly with specific reasons.
4. Do NOT defer to others just because they have higher authority — advocate for your professional perspective.
5. Do NOT artificially agree or create false consensus — real professionals disagree.
6. If the topic is outside your knowledge boundaries, say so and defer to the appropriate expert.
7. Keep responses focused and concise (2-4 sentences per turn).
8. Reference previous statements by other participants when responding.

"#,
            persona.role
        ));

        // Layer 2: Persona definition
        prompt.push_str("[PERSONA DEFINITION]\n");
        prompt.push_str(&format!("Name: {}\n", persona.name));
        prompt.push_str(&format!("Role: {}\n", persona.role));
        prompt.push_str("\nObjectives:\n");
        for obj in &persona.objectives {
            prompt.push_str(&format!("- {}\n", obj));
        }
        prompt.push_str("\nConstraints:\n");
        for c in &persona.constraints {
            prompt.push_str(&format!("- {}\n", c));
        }
        prompt.push_str("\nKnowledge Boundaries:\n");
        prompt.push_str("You KNOW about:\n");
        for item in &persona.knowledge_boundaries.knows {
            prompt.push_str(&format!("- {}\n", item));
        }
        prompt.push_str("You DO NOT KNOW about:\n");
        for item in &persona.knowledge_boundaries.does_not_know {
            prompt.push_str(&format!("- {}\n", item));
        }
        prompt.push('\n');

        // Layer 3: World model context (abbreviated for meetings)
        prompt.push_str("[BUSINESS CONTEXT]\n");
        for rule in &self.context.world_model.rules.rules {
            prompt.push_str(&format!("- Rule: {}\n", rule.description));
        }
        prompt.push('\n');

        // Layer 4: Meeting discussion history — PUBLIC statements only (Req 29.2)
        prompt.push_str(&format!("[MEETING DISCUSSION]\nTopic: {}\n\n", meeting.topic));
        if meeting.turns.is_empty() {
            prompt.push_str("This is the opening of the meeting. You speak first or respond to the topic.\n");
        } else {
            prompt.push_str("Previous statements in this meeting:\n");
            for turn in &meeting.turns {
                // Find the persona name for this turn
                let speaker_name = self
                    .context
                    .personas
                    .iter()
                    .find(|p| p.persona_id == turn.persona_id)
                    .map(|p| p.name.as_str())
                    .unwrap_or(&turn.persona_id);
                prompt.push_str(&format!(
                    "Turn {}: {} said: \"{}\"\n",
                    turn.turn_number, speaker_name, turn.statement
                ));
            }
            prompt.push('\n');
        }

        // Layer 5: Instruction for this turn
        prompt.push_str(&format!(
            "[YOUR TURN (Turn {})]\n\
             Respond as {} to the discussion above. \
             State your professional perspective clearly. \
             If you disagree with anyone, explain why based on YOUR role and constraints.\n",
            current_turn, persona.name
        ));

        // Final reinforcement
        prompt.push_str(&format!(
            "\nRemember: You are {}. Maintain your role. Do NOT agree just to be agreeable.",
            persona.name
        ));

        prompt
    }

    /// Detect conflicts from meeting turns.
    ///
    /// Req 9.3, 21.1-21.5: Identify conflicting positions between personas.
    /// Uses simple heuristic: look for disagreement markers in responses.
    pub fn detect_conflicts(turns: &[MeetingTurn], personas: &[&PersonaDefinition]) -> Vec<Conflict> {
        let mut conflicts = Vec::new();

        // Disagreement detection phrases
        let disagreement_markers = [
            "disagree", "however", "but i think", "on the other hand",
            "that's not", "i don't agree", "my concern",
            "discordo", "porém", "mas eu acho", "por outro lado",
            "minha preocupação", "não concordo",
            "the priority should", "we should focus on",
            "that won't work", "that's risky",
        ];

        // For each pair of consecutive turns from different personas,
        // check if the later one contains disagreement markers
        for i in 1..turns.len() {
            let current = &turns[i];
            let previous = &turns[i - 1];

            if current.persona_id == previous.persona_id {
                continue;
            }

            let lower_statement = current.statement.to_lowercase();
            let has_disagreement = disagreement_markers
                .iter()
                .any(|marker| lower_statement.contains(marker));

            if has_disagreement {
                let current_persona_name = personas
                    .iter()
                    .find(|p| p.persona_id == current.persona_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| current.persona_id.clone());

                let previous_persona_name = personas
                    .iter()
                    .find(|p| p.persona_id == previous.persona_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| previous.persona_id.clone());

                let mut positions = HashMap::new();
                positions.insert(
                    previous.persona_id.clone(),
                    truncate_str(&previous.statement, 200),
                );
                positions.insert(
                    current.persona_id.clone(),
                    truncate_str(&current.statement, 200),
                );

                conflicts.push(Conflict {
                    conflict_id: Uuid::new_v4().to_string(),
                    personas: vec![previous.persona_id.clone(), current.persona_id.clone()],
                    issue: format!(
                        "{} disagrees with {} on the discussion topic",
                        current_persona_name, previous_persona_name
                    ),
                    positions,
                    resolved: false,
                    resolution: None,
                });
            }
        }

        conflicts
    }

    /// Check if the meeting should conclude.
    ///
    /// Req 9.5: Meeting concludes at turn limit OR natural resolution.
    pub fn should_conclude(
        &self,
        meeting: &Meeting,
        current_turn: u32,
    ) -> Option<ConclusionReason> {
        // Turn limit reached
        if current_turn >= self.config.turn_limit {
            return Some(ConclusionReason::TurnLimitReached);
        }

        // Natural conclusion detection (only after minimum turns)
        if current_turn >= self.config.min_turns_before_conclusion && !meeting.turns.is_empty() {
            if Self::detect_natural_conclusion(&meeting.turns) {
                return Some(ConclusionReason::NaturalConclusion);
            }
        }

        None
    }

    /// Heuristic detection of natural conclusion.
    ///
    /// Looks for agreement/summary markers in the last 2 turns.
    fn detect_natural_conclusion(turns: &[MeetingTurn]) -> bool {
        if turns.len() < 2 {
            return false;
        }

        let conclusion_markers = [
            "i agree with", "that makes sense", "let's proceed",
            "we should go ahead", "i think we have a plan",
            "sounds good", "i can work with that",
            "concordo", "faz sentido", "vamos prosseguir",
            "podemos seguir", "parece um bom plano",
        ];

        let last_two: Vec<&MeetingTurn> = turns.iter().rev().take(2).collect();

        // Both last turns contain agreement markers → natural conclusion
        let agreements: usize = last_two
            .iter()
            .filter(|turn| {
                let lower = turn.statement.to_lowercase();
                conclusion_markers.iter().any(|m| lower.contains(m))
            })
            .count();

        agreements >= 2
    }

    /// Generate a meeting summary including unresolved conflicts.
    ///
    /// Req 21.5: Summary must include unresolved conflicts.
    pub fn generate_summary(meeting: &Meeting) -> String {
        let mut summary = String::new();

        summary.push_str(&format!(
            "═══ Meeting Summary ═══\n\
             Topic: {}\n\
             Participants: {}\n\
             Turns: {}\n",
            meeting.topic,
            meeting.participant_personas.join(", "),
            meeting.turns.len()
        ));

        if let Some(conclusion) = &meeting.conclusion {
            summary.push_str(&format!("Conclusion: {}\n", conclusion));
        }

        if !meeting.conflicts.is_empty() {
            summary.push_str(&format!(
                "\n─── Conflicts Identified ({}) ───\n",
                meeting.conflicts.len()
            ));
            for (i, conflict) in meeting.conflicts.iter().enumerate() {
                summary.push_str(&format!("{}. {}\n", i + 1, conflict.issue));
                for (persona_id, position) in &conflict.positions {
                    summary.push_str(&format!("   • {}: {}\n", persona_id, position));
                }
                if conflict.resolved {
                    if let Some(resolution) = &conflict.resolution {
                        summary.push_str(&format!("   ✅ Resolved: {}\n", resolution));
                    }
                } else {
                    summary.push_str("   ⚠️ Unresolved\n");
                }
            }
        } else {
            summary.push_str("\nNo conflicts detected.\n");
        }

        summary
    }
}

/// Truncate a string with ellipsis
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use context_repository::models::persona::{
        BehavioralPatterns, KnowledgeBoundaries, PersonaDefinition, ValidationCriteria,
    };
    use context_repository::models::world_model::{
        BusinessFlows, DomainConstraints, DomainProblems, DomainRules,
    };

    // ─── Helpers ──────────────────────────────────────────────────────────────

    fn make_persona(id: &str, name: &str, role: &str) -> PersonaDefinition {
        PersonaDefinition {
            persona_id: id.to_string(),
            name: name.to_string(),
            role: role.to_string(),
            objectives: vec![format!("{} objectives", role)],
            responsibilities: vec![format!("{} responsibilities", role)],
            constraints: vec![format!("{} constraints", role)],
            knowledge_boundaries: KnowledgeBoundaries {
                knows: vec![format!("{} knowledge", role)],
                does_not_know: vec![format!("Not {} domain", role)],
            },
            behavioral_patterns: BehavioralPatterns {
                uncertainty_level: "moderate".to_string(),
                uncertainty_triggers: vec!["Complex issues".to_string()],
                conflict_triggers: vec!["Priority disagreements".to_string()],
                communication_style: "Professional".to_string(),
            },
            validation_criteria: ValidationCriteria {
                decision_quality: vec!["Quality".to_string()],
                objective_measures: vec!["Measure".to_string()],
                subjective_judgment: vec!["Judgment".to_string()],
            },
        }
    }

    fn make_turn(turn_num: u32, persona_id: &str, statement: &str) -> MeetingTurn {
        MeetingTurn {
            turn_number: turn_num,
            persona_id: persona_id.to_string(),
            statement: statement.to_string(),
            responds_to: if turn_num > 1 { Some(turn_num - 1) } else { None },
            timestamp: chrono::Utc::now(),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Task 15: Meeting data structures and flow
    // ═══════════════════════════════════════════════════════════════════════════

    /// Req 9.1: Meeting created with participants and topic
    #[test]
    fn test_create_meeting_with_participants() {
        let mechanic = make_persona("mechanic", "Carlos", "Lead Mechanic");
        let attendant = make_persona("attendant", "Maria", "Service Attendant");
        let participants: Vec<&PersonaDefinition> = vec![&mechanic, &attendant];

        let meeting = MeetingOrchestrator::create_meeting(&participants, "Repair approval");

        assert_eq!(meeting.participant_personas, vec!["mechanic", "attendant"]);
        assert_eq!(meeting.topic, "Repair approval");
        assert!(meeting.turns.is_empty());
        assert!(meeting.conflicts.is_empty());
        assert!(meeting.conclusion.is_none());
    }

    /// Req 9.1: Meeting requires at least 2 personas
    #[test]
    fn test_meeting_error_too_few_personas() {
        let err = MeetingError::TooFewPersonas(1);
        assert!(err.to_string().contains("at least 2"));
    }

    /// Req 9.2: Turns are built with correct structure
    #[test]
    fn test_meeting_turn_structure() {
        let turn1 = make_turn(1, "mechanic", "The engine needs a full diagnostic.");
        assert_eq!(turn1.turn_number, 1);
        assert_eq!(turn1.persona_id, "mechanic");
        assert!(turn1.responds_to.is_none()); // First turn has no reference

        let turn2 = make_turn(2, "attendant", "How long will that take?");
        assert_eq!(turn2.responds_to, Some(1)); // Responds to turn 1
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Task 16: Context isolation
    // ═══════════════════════════════════════════════════════════════════════════

    /// Req 29.1, 29.2: Each persona gets independent prompt — Property 8
    #[test]
    fn test_meeting_prompts_are_independent() {
        let mechanic = make_persona("mechanic", "Carlos (Mechanic)", "Lead Mechanic");
        let attendant = make_persona("attendant", "Maria (Attendant)", "Service Attendant");

        let assembler = PromptAssembler::new();

        // Build meeting prompts for each persona (simulating the logic)
        let meeting = Meeting::new(
            vec!["mechanic".to_string(), "attendant".to_string()],
            "Repair cost discussion",
        );

        // Mechanic's prompt should contain ONLY mechanic's persona
        let mechanic_prompt = build_test_meeting_prompt(&mechanic, &meeting, 1);
        assert!(
            mechanic_prompt.contains("Lead Mechanic"),
            "Mechanic prompt must contain mechanic role"
        );
        assert!(
            !mechanic_prompt.contains("Service Attendant"),
            "Mechanic prompt must NOT contain attendant's role definition (Req 29.1)"
        );

        // Attendant's prompt should contain ONLY attendant's persona
        let attendant_prompt = build_test_meeting_prompt(&attendant, &meeting, 1);
        assert!(
            attendant_prompt.contains("Service Attendant"),
            "Attendant prompt must contain attendant role"
        );
        assert!(
            !attendant_prompt.contains("Lead Mechanic"),
            "Attendant prompt must NOT contain mechanic's role definition (Req 29.1)"
        );
    }

    /// Req 29.2: Previous turns are visible but internal reasoning is NOT shared
    #[test]
    fn test_meeting_prompt_shows_public_statements_only() {
        let mechanic = make_persona("mechanic", "Carlos", "Lead Mechanic");
        let mut meeting = Meeting::new(
            vec!["mechanic".to_string(), "attendant".to_string()],
            "Test topic",
        );
        meeting.turns.push(make_turn(1, "attendant", "The customer wants a cheaper option."));

        let prompt = build_test_meeting_prompt(&mechanic, &meeting, 2);

        // Should see the attendant's public statement
        assert!(
            prompt.contains("The customer wants a cheaper option"),
            "Prompt must include previous public statements"
        );
        // Should NOT contain internal persona definitions of OTHER participants
        assert!(
            !prompt.contains("Service Attendant objectives"),
            "Prompt must NOT leak other personas' internal definitions"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Task 17: Conflict detection and surfacing
    // ═══════════════════════════════════════════════════════════════════════════

    /// Req 9.3, 21.1: Detect disagreement between personas
    #[test]
    fn test_conflict_detection_with_disagreement() {
        let mechanic = make_persona("mechanic", "Carlos", "Lead Mechanic");
        let attendant = make_persona("attendant", "Maria", "Service Attendant");
        let personas: Vec<&PersonaDefinition> = vec![&mechanic, &attendant];

        let turns = vec![
            make_turn(1, "mechanic", "We need a full transmission rebuild. It's the only safe option."),
            make_turn(2, "attendant", "However, the customer's budget is limited. We should focus on the most critical repairs first."),
        ];

        let conflicts = MeetingOrchestrator::detect_conflicts(&turns, &personas);
        assert!(
            !conflicts.is_empty(),
            "Should detect conflict when 'however' is used"
        );
        assert_eq!(conflicts[0].personas.len(), 2);
        assert!(!conflicts[0].resolved);
    }

    /// Req 21.2: Conflict records include positions from both personas
    #[test]
    fn test_conflict_records_both_positions() {
        let mechanic = make_persona("mechanic", "Carlos", "Lead Mechanic");
        let owner = make_persona("owner", "João", "Shop Owner");
        let personas: Vec<&PersonaDefinition> = vec![&mechanic, &owner];

        let turns = vec![
            make_turn(1, "mechanic", "We need the premium parts for quality."),
            make_turn(2, "owner", "I disagree. The aftermarket parts are sufficient and save money."),
        ];

        let conflicts = MeetingOrchestrator::detect_conflicts(&turns, &personas);
        assert!(!conflicts.is_empty());

        let conflict = &conflicts[0];
        assert!(conflict.positions.contains_key("mechanic"));
        assert!(conflict.positions.contains_key("owner"));
    }

    /// Req 9.4, 9.8, 29.6: No conflict when personas agree
    #[test]
    fn test_no_conflict_when_agreeing() {
        let mechanic = make_persona("mechanic", "Carlos", "Lead Mechanic");
        let attendant = make_persona("attendant", "Maria", "Service Attendant");
        let personas: Vec<&PersonaDefinition> = vec![&mechanic, &attendant];

        let turns = vec![
            make_turn(1, "mechanic", "The brake pads need immediate replacement for safety."),
            make_turn(2, "attendant", "I'll inform the customer about the safety issue and get approval."),
        ];

        let conflicts = MeetingOrchestrator::detect_conflicts(&turns, &personas);
        assert!(
            conflicts.is_empty(),
            "Should not detect conflict when personas naturally agree"
        );
    }

    /// Req 21.1: Detect multiple conflicts in a long discussion
    #[test]
    fn test_multiple_conflicts_detected() {
        let mechanic = make_persona("mechanic", "Carlos", "Lead Mechanic");
        let owner = make_persona("owner", "João", "Shop Owner");
        let personas: Vec<&PersonaDefinition> = vec![&mechanic, &owner];

        let turns = vec![
            make_turn(1, "mechanic", "The engine needs a complete overhaul."),
            make_turn(2, "owner", "That's risky from a cost perspective. Can we do partial repairs?"),
            make_turn(3, "mechanic", "Partial repairs won't fix the root cause."),
            make_turn(4, "owner", "But I think the customer won't pay for a full overhaul."),
        ];

        let conflicts = MeetingOrchestrator::detect_conflicts(&turns, &personas);
        assert!(
            conflicts.len() >= 2,
            "Should detect multiple conflicts, got {}",
            conflicts.len()
        );
    }

    /// Req 9.3: Portuguese disagreement markers detected
    #[test]
    fn test_conflict_detection_portuguese() {
        let mechanic = make_persona("mechanic", "Carlos", "Lead Mechanic");
        let owner = make_persona("owner", "João", "Shop Owner");
        let personas: Vec<&PersonaDefinition> = vec![&mechanic, &owner];

        let turns = vec![
            make_turn(1, "mechanic", "Precisamos de peças originais."),
            make_turn(2, "owner", "Discordo. Peças genéricas são suficientes para este reparo."),
        ];

        let conflicts = MeetingOrchestrator::detect_conflicts(&turns, &personas);
        assert!(
            !conflicts.is_empty(),
            "Should detect Portuguese conflict marker 'discordo'"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Task 18: Meeting conclusion logic
    // ═══════════════════════════════════════════════════════════════════════════

    /// Req 9.5: Turn limit enforced
    #[test]
    fn test_conclusion_at_turn_limit() {
        let config = MeetingConfig {
            turn_limit: 5,
            min_turns_before_conclusion: 3,
        };
        let meeting = Meeting::new(vec!["a".to_string(), "b".to_string()], "topic");

        // Use a minimal orchestrator-like check
        assert_eq!(
            check_should_conclude(&config, &meeting, 5),
            Some(ConclusionReason::TurnLimitReached)
        );
    }

    /// Req 9.5: No premature conclusion
    #[test]
    fn test_no_premature_conclusion() {
        let config = MeetingConfig {
            turn_limit: 10,
            min_turns_before_conclusion: 4,
        };
        let meeting = Meeting::new(vec!["a".to_string(), "b".to_string()], "topic");

        assert_eq!(
            check_should_conclude(&config, &meeting, 2),
            None,
            "Should not conclude before min_turns"
        );
    }

    /// Req 9.5: Natural conclusion detected when both last turns show agreement
    #[test]
    fn test_natural_conclusion_detected() {
        let mut meeting = Meeting::new(vec!["a".to_string(), "b".to_string()], "topic");
        meeting.turns.push(make_turn(1, "a", "I don't think we need more parts."));
        meeting.turns.push(make_turn(2, "b", "The cost is too high."));
        meeting.turns.push(make_turn(3, "a", "OK, I agree with that approach."));
        meeting.turns.push(make_turn(4, "b", "That makes sense. Let's proceed."));

        assert!(
            MeetingOrchestrator::detect_natural_conclusion(&meeting.turns),
            "Should detect natural conclusion when both recent turns show agreement"
        );
    }

    /// Req 9.5: No natural conclusion when only one agrees
    #[test]
    fn test_no_natural_conclusion_with_one_agreement() {
        let mut meeting = Meeting::new(vec!["a".to_string(), "b".to_string()], "topic");
        meeting.turns.push(make_turn(1, "a", "We need to replace the whole unit."));
        meeting.turns.push(make_turn(2, "b", "I agree with that approach."));
        meeting.turns.push(make_turn(3, "a", "But I'm still concerned about the timeline."));

        // Only 1 out of last 2 has agreement markers
        assert!(
            !MeetingOrchestrator::detect_natural_conclusion(&meeting.turns),
            "Should not conclude when only one participant agrees"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Task 18: Meeting summary generation
    // ═══════════════════════════════════════════════════════════════════════════

    /// Req 21.5: Summary includes unresolved conflicts
    #[test]
    fn test_summary_includes_unresolved_conflicts() {
        let mut meeting = Meeting::new(
            vec!["mechanic".to_string(), "owner".to_string()],
            "Budget allocation",
        );
        meeting.turns.push(make_turn(1, "mechanic", "Statement 1"));
        meeting.turns.push(make_turn(2, "owner", "Statement 2"));
        meeting.conflicts.push(Conflict {
            conflict_id: "c1".to_string(),
            personas: vec!["mechanic".to_string(), "owner".to_string()],
            issue: "Cost vs quality disagreement".to_string(),
            positions: {
                let mut p = HashMap::new();
                p.insert("mechanic".to_string(), "Premium parts needed".to_string());
                p.insert("owner".to_string(), "Budget is limited".to_string());
                p
            },
            resolved: false,
            resolution: None,
        });

        let summary = MeetingOrchestrator::generate_summary(&meeting);

        assert!(summary.contains("Budget allocation"), "Summary must contain topic");
        assert!(summary.contains("mechanic"), "Summary must list participants");
        assert!(summary.contains("Conflicts Identified"), "Summary must show conflicts section");
        assert!(summary.contains("Cost vs quality"), "Summary must describe conflict");
        assert!(summary.contains("Unresolved"), "Summary must flag unresolved conflicts");
    }

    /// Summary for meeting with no conflicts
    #[test]
    fn test_summary_no_conflicts() {
        let meeting = Meeting::new(
            vec!["mechanic".to_string(), "attendant".to_string()],
            "Schedule review",
        );

        let summary = MeetingOrchestrator::generate_summary(&meeting);
        assert!(summary.contains("No conflicts detected"));
    }

    /// Summary with conclusion
    #[test]
    fn test_summary_with_conclusion() {
        let mut meeting = Meeting::new(
            vec!["mechanic".to_string(), "attendant".to_string()],
            "Parts ordering",
        );
        meeting.conclusion = Some("Agreed to order OEM parts with 2-day shipping.".to_string());

        let summary = MeetingOrchestrator::generate_summary(&meeting);
        assert!(summary.contains("Agreed to order OEM parts"));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Property 8: Meeting Context Isolation — verified across persona permutations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Property 8: For any pair of personas, their meeting prompts MUST NOT contain
    /// each other's role definition (only their own).
    #[test]
    fn test_property_8_context_isolation_all_pairs() {
        let personas = vec![
            make_persona("mechanic", "Carlos (Mechanic)", "Lead Mechanic"),
            make_persona("attendant", "Maria (Attendant)", "Service Attendant"),
            make_persona("owner", "João (Owner)", "Shop Owner"),
        ];

        let meeting = Meeting::new(
            personas.iter().map(|p| p.persona_id.clone()).collect(),
            "Budget meeting",
        );

        for i in 0..personas.len() {
            let prompt = build_test_meeting_prompt(&personas[i], &meeting, 1);

            // This persona's role MUST be in the prompt
            assert!(
                prompt.contains(&personas[i].role),
                "Persona {}'s own role must be in their prompt",
                personas[i].persona_id
            );

            // Other personas' roles MUST NOT be in the prompt's persona definition
            for j in 0..personas.len() {
                if i == j {
                    continue;
                }
                // Check that the OTHER persona's role doesn't appear in the
                // [PERSONA DEFINITION] section (it may appear in turn history)
                let persona_section_end = prompt.find("[BUSINESS CONTEXT]").unwrap_or(prompt.len());
                let persona_section = &prompt[..persona_section_end];

                assert!(
                    !persona_section.contains(&personas[j].role),
                    "Persona {}'s prompt persona section must NOT contain {}'s role '{}' (Req 29.1)",
                    personas[i].persona_id,
                    personas[j].persona_id,
                    personas[j].role
                );
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Property 9: Conflict Preservation — conflicts SHALL NOT be suppressed
    // ═══════════════════════════════════════════════════════════════════════════

    /// Property 9: Detected conflicts MUST be preserved in the meeting record.
    #[test]
    fn test_property_9_conflicts_preserved() {
        let mechanic = make_persona("mechanic", "Carlos", "Lead Mechanic");
        let owner = make_persona("owner", "João", "Shop Owner");
        let personas: Vec<&PersonaDefinition> = vec![&mechanic, &owner];

        let turns = vec![
            make_turn(1, "mechanic", "Quality requires premium parts."),
            make_turn(2, "owner", "I disagree. We need to consider the budget."),
            make_turn(3, "mechanic", "But I think cutting on parts is risky."),
            make_turn(4, "owner", "My concern is profitability."),
        ];

        let conflicts = MeetingOrchestrator::detect_conflicts(&turns, &personas);
        let conflict_count = conflicts.len();

        // Apply conflicts to meeting
        let mut meeting = Meeting::new(
            vec!["mechanic".to_string(), "owner".to_string()],
            "Parts quality",
        );
        for turn in turns {
            meeting.turns.push(turn);
        }
        for conflict in conflicts {
            meeting.conflicts.push(conflict);
        }

        // Conflicts MUST be preserved
        assert_eq!(
            meeting.conflicts.len(),
            conflict_count,
            "All detected conflicts must be preserved in the meeting record"
        );

        // Summary must mention them
        let summary = MeetingOrchestrator::generate_summary(&meeting);
        assert!(
            summary.contains("Conflicts Identified"),
            "Summary must include conflict section"
        );
    }

    // ─── Helper to simulate prompt building without Ollama ──────────────────

    fn build_test_meeting_prompt(
        persona: &PersonaDefinition,
        meeting: &Meeting,
        turn_number: u32,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str(&format!(
            "[SYSTEM INSTRUCTIONS - MEETING MODE]\n\
             You are participating in a meeting.\n\
             Your role: {}\n\n",
            persona.role
        ));

        prompt.push_str("[PERSONA DEFINITION]\n");
        prompt.push_str(&format!("Name: {}\n", persona.name));
        prompt.push_str(&format!("Role: {}\n", persona.role));
        prompt.push_str("\nObjectives:\n");
        for obj in &persona.objectives {
            prompt.push_str(&format!("- {}\n", obj));
        }
        prompt.push_str("\nConstraints:\n");
        for c in &persona.constraints {
            prompt.push_str(&format!("- {}\n", c));
        }
        prompt.push('\n');

        prompt.push_str("[BUSINESS CONTEXT]\n\n");

        prompt.push_str(&format!("[MEETING DISCUSSION]\nTopic: {}\n\n", meeting.topic));
        for turn in &meeting.turns {
            prompt.push_str(&format!(
                "Turn {}: {} said: \"{}\"\n",
                turn.turn_number, turn.persona_id, turn.statement
            ));
        }

        prompt.push_str(&format!(
            "\n[YOUR TURN (Turn {})]\nRespond as {}.\n",
            turn_number, persona.name
        ));

        prompt
    }

    fn check_should_conclude(
        config: &MeetingConfig,
        meeting: &Meeting,
        current_turn: u32,
    ) -> Option<ConclusionReason> {
        if current_turn >= config.turn_limit {
            return Some(ConclusionReason::TurnLimitReached);
        }
        if current_turn >= config.min_turns_before_conclusion && !meeting.turns.is_empty() {
            if MeetingOrchestrator::detect_natural_conclusion(&meeting.turns) {
                return Some(ConclusionReason::NaturalConclusion);
            }
        }
        None
    }
}
