//! Behavioral Consistency Validation — Task 22
//!
//! Analyzes a persona's responses across a session to detect:
//! 1. Contradictions — directly conflicting statements without justification
//! 2. Priority drift — shift in emphasis away from persona's core objectives
//! 3. Constraint violations — recommendations that violate persona constraints
//!
//! Requirements: 36.1–36.6

use std::collections::HashSet;

use chrono::Utc;
use context_repository::models::persona::PersonaDefinition;
use context_repository::models::quality::{ConsistencyReport, Contradiction};
use context_repository::models::session::Session;

/// Validates behavioral consistency of a persona across a session.
pub struct ConsistencyValidator;

impl ConsistencyValidator {
    /// Analyze all consultations for a given persona within a session.
    ///
    /// Req 36.1: Detect contradictions across consultations.
    /// Req 36.3: Detect priority drift.
    /// Req 36.4: Detect alignment with persona definition.
    pub fn validate(
        session: &Session,
        persona: &PersonaDefinition,
    ) -> ConsistencyReport {
        // Filter consultations for this persona
        let persona_consultations: Vec<_> = session
            .consultation_history
            .iter()
            .filter(|c| c.persona_id == persona.persona_id)
            .collect();

        let analyzed = persona_consultations.len() as u32;

        // 1. Detect contradictions (Req 36.2)
        let contradictions = Self::detect_contradictions(&persona_consultations);

        // 2. Detect priority drift (Req 36.3)
        let priority_drift_score = Self::calculate_priority_drift(
            &persona_consultations,
            persona,
        );

        // 3. Overall consistency score
        let overall = Self::calculate_overall_score(
            analyzed,
            contradictions.len() as u32,
            priority_drift_score,
        );

        ConsistencyReport {
            session_id: session.session_id.clone(),
            persona_id: persona.persona_id.clone(),
            generated_at: Utc::now(),
            consultations_analyzed: analyzed,
            contradictions,
            priority_drift_score,
            overall_consistency_score: overall,
        }
    }

    /// Detect contradictions between consultation responses.
    ///
    /// Req 36.2: A contradiction is when a response affirms something
    /// that a previous response denied (or vice versa), without providing
    /// new context or justification.
    fn detect_contradictions(
        consultations: &[&context_repository::models::session::Consultation],
    ) -> Vec<Contradiction> {
        let mut contradictions = Vec::new();

        // Contradiction pairs: (affirmative, negative)
        let contradiction_pairs = [
            ("recommend", "do not recommend"),
            ("necessary", "not necessary"),
            ("required", "not required"),
            ("safe", "not safe"),
            ("approve", "cannot approve"),
            ("should", "should not"),
            ("need to", "don't need to"),
            ("can be", "cannot be"),
            ("is possible", "is not possible"),
            ("recomendo", "não recomendo"),
            ("necessário", "não é necessário"),
            ("seguro", "não é seguro"),
        ];

        // Justification markers — presence exempts a contradiction
        let justification_markers = [
            "however, given", "after further", "new information",
            "upon review", "considering the", "based on what",
            "since you", "after", "now that",
            "porém, considerando", "após análise",
        ];

        for i in 0..consultations.len() {
            for j in (i + 1)..consultations.len() {
                let resp_a = consultations[i].expert_response.to_lowercase();
                let resp_b = consultations[j].expert_response.to_lowercase();

                for (affirm, deny) in &contradiction_pairs {
                    let a_affirms = resp_a.contains(affirm) && !resp_a.contains(deny);
                    let b_denies = resp_b.contains(deny);

                    let a_denies = resp_a.contains(deny);
                    let b_affirms = resp_b.contains(affirm) && !resp_b.contains(deny);

                    if (a_affirms && b_denies) || (a_denies && b_affirms) {
                        // Check if B has justification
                        let has_justification = justification_markers
                            .iter()
                            .any(|m| resp_b.contains(m));

                        contradictions.push(Contradiction {
                            consultation_a_id: consultations[i].consultation_id.clone(),
                            consultation_b_id: consultations[j].consultation_id.clone(),
                            issue: format!(
                                "Contradiction on '{}' / '{}'",
                                affirm, deny
                            ),
                            statement_a: truncate(&consultations[i].expert_response, 150),
                            statement_b: truncate(&consultations[j].expert_response, 150),
                            justified: has_justification,
                        });

                        break; // One contradiction per pair
                    }
                }
            }
        }

        contradictions
    }

    /// Calculate priority drift score.
    ///
    /// Req 36.3: Measures how well responses align with the persona's
    /// stated objectives. Lower drift = better consistency.
    ///
    /// Score 0.0 = perfect alignment, 1.0 = complete drift.
    fn calculate_priority_drift(
        consultations: &[&context_repository::models::session::Consultation],
        persona: &PersonaDefinition,
    ) -> f64 {
        if consultations.is_empty() || persona.objectives.is_empty() {
            return 0.0;
        }

        // Extract keywords from objectives
        let objective_keywords: HashSet<String> = persona
            .objectives
            .iter()
            .flat_map(|obj| {
                obj.to_lowercase()
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .map(|w| w.to_string())
                    .collect::<Vec<_>>()
            })
            .collect();

        // Measure keyword presence in each response
        let alignment_scores: Vec<f64> = consultations
            .iter()
            .map(|c| {
                let lower_resp = c.expert_response.to_lowercase();
                let hits = objective_keywords
                    .iter()
                    .filter(|kw| lower_resp.contains(kw.as_str()))
                    .count();

                if objective_keywords.is_empty() {
                    1.0
                } else {
                    hits as f64 / objective_keywords.len() as f64
                }
            })
            .collect();

        if alignment_scores.is_empty() {
            return 0.0;
        }

        // Drift = 1.0 - average alignment
        let avg_alignment: f64 = alignment_scores.iter().sum::<f64>() / alignment_scores.len() as f64;
        (1.0 - avg_alignment).max(0.0).min(1.0)
    }

    /// Calculate overall consistency score.
    ///
    /// Combines contradiction rate and priority drift.
    /// Score 1.0 = perfectly consistent, 0.0 = fully inconsistent.
    fn calculate_overall_score(
        consultations_analyzed: u32,
        contradiction_count: u32,
        priority_drift: f64,
    ) -> f64 {
        if consultations_analyzed == 0 {
            return 1.0; // Default: no data = consistent
        }

        // Contradiction penalty: each contradiction reduces score
        let max_possible_contradictions = (consultations_analyzed * (consultations_analyzed - 1)) / 2;
        let contradiction_rate = if max_possible_contradictions > 0 {
            contradiction_count as f64 / max_possible_contradictions as f64
        } else {
            0.0
        };

        // Overall = weighted average
        let consistency = 1.0 - (0.6 * contradiction_rate + 0.4 * priority_drift);
        consistency.max(0.0).min(1.0)
    }

    /// Format a human-readable consistency report.
    pub fn format_report(report: &ConsistencyReport) -> String {
        let mut out = String::new();

        out.push_str(&format!(
            "═══ Consistency Report: {} ═══\n",
            report.persona_id
        ));
        out.push_str(&format!(
            "Consultations analyzed: {}\n",
            report.consultations_analyzed
        ));
        out.push_str(&format!(
            "Overall consistency: {:.0}%\n",
            report.overall_consistency_score * 100.0
        ));
        out.push_str(&format!(
            "Priority drift: {:.0}%\n",
            report.priority_drift_score * 100.0
        ));

        if !report.contradictions.is_empty() {
            out.push_str(&format!(
                "\n─── Contradictions ({}) ───\n",
                report.contradictions.len()
            ));
            for (i, c) in report.contradictions.iter().enumerate() {
                out.push_str(&format!("{}. {}\n", i + 1, c.issue));
                out.push_str(&format!("   A: {}\n", c.statement_a));
                out.push_str(&format!("   B: {}\n", c.statement_b));
                if c.justified {
                    out.push_str("   ✅ Justified (new context provided)\n");
                } else {
                    out.push_str("   ⚠️ Unjustified\n");
                }
            }
        } else {
            out.push_str("\nNo contradictions detected.\n");
        }

        out
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_repository::models::persona::{
        BehavioralPatterns, KnowledgeBoundaries, ValidationCriteria,
    };
    use context_repository::models::session::{Consultation, Session};

    fn make_persona() -> PersonaDefinition {
        PersonaDefinition {
            persona_id: "mechanic".to_string(),
            name: "Carlos".to_string(),
            role: "Lead Mechanic".to_string(),
            objectives: vec![
                "Diagnose vehicle problems accurately".to_string(),
                "Recommend necessary repairs".to_string(),
            ],
            responsibilities: vec!["Diagnostics".to_string()],
            constraints: vec!["Cannot approve > $500".to_string()],
            knowledge_boundaries: KnowledgeBoundaries {
                knows: vec!["Mechanical systems".to_string()],
                does_not_know: vec!["Finances".to_string()],
            },
            behavioral_patterns: BehavioralPatterns {
                uncertainty_level: "moderate".to_string(),
                uncertainty_triggers: vec!["Intermittent".to_string()],
                conflict_triggers: vec!["Cost".to_string()],
                communication_style: "Direct".to_string(),
            },
            validation_criteria: ValidationCriteria {
                decision_quality: vec!["Thoroughness".to_string()],
                objective_measures: vec!["Root cause".to_string()],
                subjective_judgment: vec!["Priority".to_string()],
            },
        }
    }

    fn make_session() -> Session {
        Session::new("auto-repair", "1.0.0", "diagnostic")
    }

    fn make_consultation(persona_id: &str, query: &str, response: &str) -> Consultation {
        Consultation::new(persona_id, query, response, 1000)
    }

    // ─── Contradiction detection ──────────────────────────────────────────

    /// Req 36.2: Detect direct contradiction
    #[test]
    fn test_detects_contradiction() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "Should we replace the belt?",
            "I recommend replacing the belt immediately."
        ));
        session.record_consultation(make_consultation(
            "mechanic", "About that belt...",
            "I do not recommend replacing the belt at this time."
        ));

        let report = ConsistencyValidator::validate(&session, &make_persona());
        assert!(
            !report.contradictions.is_empty(),
            "Should detect 'recommend' vs 'do not recommend' contradiction"
        );
    }

    /// Req 36.2: Justified contradiction (with new context) is flagged but marked
    #[test]
    fn test_justified_contradiction_marked() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "Is it safe?",
            "The repair is safe to proceed with."
        ));
        session.record_consultation(make_consultation(
            "mechanic", "What about the crack?",
            "After further inspection, the repair is not safe. The frame has a crack."
        ));

        let report = ConsistencyValidator::validate(&session, &make_persona());
        assert!(!report.contradictions.is_empty());
        assert!(
            report.contradictions[0].justified,
            "Contradiction with 'after further' should be marked as justified"
        );
    }

    /// No contradiction when responses are consistent
    #[test]
    fn test_no_contradiction_when_consistent() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "What about the brakes?",
            "The brake pads need replacement."
        ));
        session.record_consultation(make_consultation(
            "mechanic", "Anything else?",
            "The rotors also show wear. I recommend both pads and rotors."
        ));

        let report = ConsistencyValidator::validate(&session, &make_persona());
        assert!(
            report.contradictions.is_empty(),
            "Consistent responses should have no contradictions"
        );
    }

    /// Only analyze the target persona's consultations
    #[test]
    fn test_only_analyzes_target_persona() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "Belt?", "I recommend replacing."
        ));
        session.record_consultation(make_consultation(
            "attendant", "Belt?", "I do not recommend replacing."
        ));

        let report = ConsistencyValidator::validate(&session, &make_persona());
        assert_eq!(report.consultations_analyzed, 1, "Only mechanic consultations");
        assert!(
            report.contradictions.is_empty(),
            "Cross-persona contradiction should not be reported"
        );
    }

    // ─── Priority drift ──────────────────────────────────────────────────

    /// Req 36.3: Low drift when responses align with objectives
    #[test]
    fn test_low_priority_drift_when_aligned() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "What's wrong?",
            "I need to diagnose the vehicle problems accurately before recommending any repairs."
        ));
        session.record_consultation(make_consultation(
            "mechanic", "Next steps?",
            "The necessary repairs include replacing the worn components."
        ));

        let report = ConsistencyValidator::validate(&session, &make_persona());
        assert!(
            report.priority_drift_score < 0.5,
            "Responses mentioning objectives should have low drift, got {}",
            report.priority_drift_score
        );
    }

    /// Req 36.3: High drift when responses diverge from objectives
    #[test]
    fn test_high_priority_drift_when_diverging() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "Topic?",
            "The weather has been really nice lately."
        ));
        session.record_consultation(make_consultation(
            "mechanic", "More?",
            "I enjoy cooking on weekends."
        ));

        let report = ConsistencyValidator::validate(&session, &make_persona());
        assert!(
            report.priority_drift_score > 0.7,
            "Completely off-topic responses should have high drift, got {}",
            report.priority_drift_score
        );
    }

    // ─── Overall scoring ──────────────────────────────────────────────────

    /// Req 36.5: Empty session = perfect consistency (default)
    #[test]
    fn test_empty_session_perfect_consistency() {
        let session = make_session();
        let report = ConsistencyValidator::validate(&session, &make_persona());
        assert_eq!(report.consultations_analyzed, 0);
        assert_eq!(report.overall_consistency_score, 1.0);
    }

    /// Req 36.5: Good consistency score when aligned and no contradictions
    #[test]
    fn test_good_overall_score() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "Problem?",
            "I need to diagnose and recommend necessary repairs."
        ));

        let report = ConsistencyValidator::validate(&session, &make_persona());
        assert!(
            report.overall_consistency_score > 0.5,
            "Single aligned consultation should have good score"
        );
    }

    // ─── Report formatting ────────────────────────────────────────────────

    /// Req 36.5: Formatted report contains key sections
    #[test]
    fn test_format_report() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "Q", "I recommend the repair."
        ));
        session.record_consultation(make_consultation(
            "mechanic", "Q2", "I do not recommend the repair now."
        ));

        let report = ConsistencyValidator::validate(&session, &make_persona());
        let formatted = ConsistencyValidator::format_report(&report);

        assert!(formatted.contains("Consistency Report"));
        assert!(formatted.contains("mechanic"));
        assert!(formatted.contains("Contradictions"));
    }
}
