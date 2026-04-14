//! Session Quality Collector — Task 13
//!
//! Aggregates per-consultation quality metrics into session-level statistics.
//! Generates quality reports with per-persona breakdowns.
//!
//! Requirements: 34.1–34.6

use std::collections::HashMap;

use chrono::Utc;
use context_repository::models::quality::{PersonaQualityMetrics, QualityReport};
use context_repository::models::session::{QualityMetrics, QualityScores, Session, SessionMetrics};

/// Collects and aggregates quality metrics from consultations within a session.
///
/// This struct is stateless — metrics are computed on-the-fly from session data.
pub struct SessionQualityCollector;

impl SessionQualityCollector {
    /// Update session-level aggregate metrics after a consultation is recorded.
    ///
    /// Call this after `session.record_consultation()` to keep `session.metrics`
    /// in sync with the latest quality data.
    ///
    /// Req 34.5: session-level metrics aggregation
    pub fn update_session_metrics(session: &mut Session) {
        let metrics = &mut session.metrics;

        let consultations_with_quality: Vec<&QualityMetrics> = session
            .consultation_history
            .iter()
            .filter_map(|c| c.quality_metrics.as_ref())
            .collect();

        let total = consultations_with_quality.len() as f64;
        if total == 0.0 {
            return;
        }

        // Req 34.1: Average response time
        let total_time: f64 = consultations_with_quality
            .iter()
            .map(|m| m.response_time_ms as f64)
            .sum();
        metrics.average_response_time_ms = total_time / total;

        // Req 34.2-34.4: Quality score aggregates
        let uncertainty_count = consultations_with_quality
            .iter()
            .filter(|m| m.uncertainty_expressed)
            .count() as f64;

        let redirection_count = consultations_with_quality
            .iter()
            .filter(|m| m.redirected_appropriately)
            .count() as f64;

        let avg_hallucination: f64 = consultations_with_quality
            .iter()
            .map(|m| m.hallucination_score)
            .sum::<f64>()
            / total;

        let avg_fidelity: f64 = consultations_with_quality
            .iter()
            .map(|m| m.fidelity_score)
            .sum::<f64>()
            / total;

        metrics.quality_scores = QualityScores {
            average_hallucination: avg_hallucination,
            average_fidelity: avg_fidelity,
            uncertainty_rate: uncertainty_count / total,
            redirection_rate: redirection_count / total,
        };
    }

    /// Generate a comprehensive quality report for the session.
    ///
    /// Req 34.5, 34.6: Generate quality report with per-persona breakdown.
    pub fn generate_report(session: &Session) -> QualityReport {
        let consultations_with_quality: Vec<(&str, &QualityMetrics)> = session
            .consultation_history
            .iter()
            .filter_map(|c| c.quality_metrics.as_ref().map(|q| (c.persona_id.as_str(), q)))
            .collect();

        let total = consultations_with_quality.len() as f64;

        // Per-persona aggregation
        let mut per_persona: HashMap<String, PersonaQualityMetrics> = HashMap::new();

        for (persona_id, metrics) in &consultations_with_quality {
            let entry = per_persona
                .entry(persona_id.to_string())
                .or_insert_with(|| PersonaQualityMetrics {
                    persona_id: persona_id.to_string(),
                    consultations: 0,
                    average_response_time_ms: 0.0,
                    average_response_length: 0.0,
                    hallucination_score: 0.0,
                    fidelity_score: 0.0,
                    uncertainty_expressed_count: 0,
                    boundary_violations: 0,
                });

            entry.consultations += 1;
            entry.average_response_time_ms += metrics.response_time_ms as f64;
            entry.average_response_length += metrics.response_length as f64;
            entry.hallucination_score += metrics.hallucination_score;
            entry.fidelity_score += metrics.fidelity_score;

            if metrics.uncertainty_expressed {
                entry.uncertainty_expressed_count += 1;
            }
            if !metrics.knowledge_boundary_respected {
                entry.boundary_violations += 1;
            }
        }

        // Finalize averages
        for entry in per_persona.values_mut() {
            let count = entry.consultations as f64;
            if count > 0.0 {
                entry.average_response_time_ms /= count;
                entry.average_response_length /= count;
                entry.hallucination_score /= count;
                entry.fidelity_score /= count;
            }
        }

        // Session-level aggregates
        let (avg_hallucination, avg_fidelity, uncertainty_rate, redirection_rate, history_rate) =
            if total > 0.0 {
                let h: f64 = consultations_with_quality
                    .iter()
                    .map(|(_, m)| m.hallucination_score)
                    .sum::<f64>()
                    / total;
                let f: f64 = consultations_with_quality
                    .iter()
                    .map(|(_, m)| m.fidelity_score)
                    .sum::<f64>()
                    / total;
                let u = consultations_with_quality
                    .iter()
                    .filter(|(_, m)| m.uncertainty_expressed)
                    .count() as f64
                    / total;
                let r = consultations_with_quality
                    .iter()
                    .filter(|(_, m)| m.redirected_appropriately)
                    .count() as f64
                    / total;
                let hr = consultations_with_quality
                    .iter()
                    .filter(|(_, m)| m.history_referenced)
                    .count() as f64
                    / total;
                (h, f, u, r, hr)
            } else {
                (0.0, 1.0, 0.0, 0.0, 0.0)
            };

        QualityReport {
            session_id: session.session_id.clone(),
            generated_at: Utc::now(),
            total_consultations: consultations_with_quality.len() as u32,
            average_hallucination_score: avg_hallucination,
            average_fidelity_score: avg_fidelity,
            uncertainty_rate,
            redirection_rate,
            history_reference_rate: history_rate,
            per_persona_metrics: per_persona,
        }
    }

    /// Format a human-readable quality summary for CLI display.
    pub fn format_summary(report: &QualityReport) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "═══ Quality Report (Session {}) ═══\n",
            &report.session_id[..8.min(report.session_id.len())]
        ));
        out.push_str(&format!(
            "Total consultations analyzed: {}\n",
            report.total_consultations
        ));
        out.push_str(&format!(
            "Avg hallucination score:  {:.2} (lower is better)\n",
            report.average_hallucination_score
        ));
        out.push_str(&format!(
            "Avg fidelity score:       {:.2} (1.0 = perfect)\n",
            report.average_fidelity_score
        ));
        out.push_str(&format!(
            "Uncertainty rate:         {:.0}%\n",
            report.uncertainty_rate * 100.0
        ));
        out.push_str(&format!(
            "Redirection rate:         {:.0}%\n",
            report.redirection_rate * 100.0
        ));
        out.push_str(&format!(
            "History reference rate:   {:.0}%\n",
            report.history_reference_rate * 100.0
        ));

        if !report.per_persona_metrics.is_empty() {
            out.push_str("\n─── Per-Persona Breakdown ───\n");
            for (id, m) in &report.per_persona_metrics {
                out.push_str(&format!(
                    "\n  {} ({} consultations):\n",
                    id, m.consultations
                ));
                out.push_str(&format!(
                    "    Avg response time:  {:.0}ms\n",
                    m.average_response_time_ms
                ));
                out.push_str(&format!(
                    "    Avg response length: {:.0} chars\n",
                    m.average_response_length
                ));
                out.push_str(&format!(
                    "    Hallucination:      {:.2}\n",
                    m.hallucination_score
                ));
                out.push_str(&format!("    Fidelity:           {:.2}\n", m.fidelity_score));
                out.push_str(&format!(
                    "    Uncertainty count:  {}\n",
                    m.uncertainty_expressed_count
                ));
                out.push_str(&format!(
                    "    Boundary violations: {}\n",
                    m.boundary_violations
                ));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_repository::models::session::{Consultation, Session};

    fn make_session() -> Session {
        Session::new("auto-repair-shop", "1.0.0", "diagnostic-challenge")
    }

    fn make_consultation(
        persona_id: &str,
        query: &str,
        response: &str,
        time_ms: u64,
        quality: QualityMetrics,
    ) -> Consultation {
        let mut c = Consultation::new(persona_id, query, response, time_ms);
        c.quality_metrics = Some(quality);
        c
    }

    fn metrics_uncertain() -> QualityMetrics {
        QualityMetrics {
            response_time_ms: 1200,
            response_length: 150,
            uncertainty_expressed: true,
            knowledge_boundary_respected: true,
            history_referenced: false,
            redirected_appropriately: false,
            hallucination_score: 0.1,
            fidelity_score: 0.95,
        }
    }

    fn metrics_confident() -> QualityMetrics {
        QualityMetrics {
            response_time_ms: 800,
            response_length: 100,
            uncertainty_expressed: false,
            knowledge_boundary_respected: true,
            history_referenced: true,
            redirected_appropriately: false,
            hallucination_score: 0.0,
            fidelity_score: 1.0,
        }
    }

    fn metrics_redirect() -> QualityMetrics {
        QualityMetrics {
            response_time_ms: 600,
            response_length: 80,
            uncertainty_expressed: false,
            knowledge_boundary_respected: true,
            history_referenced: false,
            redirected_appropriately: true,
            hallucination_score: 0.0,
            fidelity_score: 1.0,
        }
    }

    fn metrics_boundary_violation() -> QualityMetrics {
        QualityMetrics {
            response_time_ms: 1000,
            response_length: 200,
            uncertainty_expressed: false,
            knowledge_boundary_respected: false,
            history_referenced: false,
            redirected_appropriately: false,
            hallucination_score: 0.3,
            fidelity_score: 0.7,
        }
    }

    // ─── update_session_metrics tests ─────────────────────────────────────────

    /// Req 34.1: Average response time is computed correctly
    #[test]
    fn test_update_metrics_average_response_time() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "q1", "r1", 1200, metrics_uncertain(),
        ));
        session.record_consultation(make_consultation(
            "mechanic", "q2", "r2", 800, metrics_confident(),
        ));
        SessionQualityCollector::update_session_metrics(&mut session);

        assert!(
            (session.metrics.average_response_time_ms - 1000.0).abs() < 0.01,
            "Average of 1200 and 800 should be 1000.0, got {}",
            session.metrics.average_response_time_ms
        );
    }

    /// Req 34.3: Uncertainty rate is computed correctly
    #[test]
    fn test_update_metrics_uncertainty_rate() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "q1", "r1", 1200, metrics_uncertain(),
        ));
        session.record_consultation(make_consultation(
            "mechanic", "q2", "r2", 800, metrics_confident(),
        ));
        SessionQualityCollector::update_session_metrics(&mut session);

        assert!(
            (session.metrics.quality_scores.uncertainty_rate - 0.5).abs() < 0.01,
            "1 out of 2 uncertain = 50%, got {}",
            session.metrics.quality_scores.uncertainty_rate
        );
    }

    /// Req 34.4: Redirection rate is computed correctly
    #[test]
    fn test_update_metrics_redirection_rate() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "q1", "r1", 600, metrics_redirect(),
        ));
        session.record_consultation(make_consultation(
            "mechanic", "q2", "r2", 800, metrics_confident(),
        ));
        session.record_consultation(make_consultation(
            "attendant", "q3", "r3", 600, metrics_redirect(),
        ));
        SessionQualityCollector::update_session_metrics(&mut session);

        let expected = 2.0 / 3.0;
        assert!(
            (session.metrics.quality_scores.redirection_rate - expected).abs() < 0.01,
            "2 out of 3 redirections = {:.2}%, got {}",
            expected * 100.0,
            session.metrics.quality_scores.redirection_rate
        );
    }

    /// Req 34.2: Hallucination and fidelity averages computed correctly
    #[test]
    fn test_update_metrics_hallucination_fidelity_averages() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "q1", "r1", 1000, metrics_uncertain(), // hallucination: 0.1, fidelity: 0.95
        ));
        session.record_consultation(make_consultation(
            "mechanic", "q2", "r2", 1000, metrics_boundary_violation(), // hallucination: 0.3, fidelity: 0.7
        ));
        SessionQualityCollector::update_session_metrics(&mut session);

        let expected_h = (0.1 + 0.3) / 2.0; // 0.2
        let expected_f = (0.95 + 0.7) / 2.0; // 0.825
        assert!(
            (session.metrics.quality_scores.average_hallucination - expected_h).abs() < 0.01,
            "Expected hallucination {}, got {}",
            expected_h,
            session.metrics.quality_scores.average_hallucination
        );
        assert!(
            (session.metrics.quality_scores.average_fidelity - expected_f).abs() < 0.01,
            "Expected fidelity {}, got {}",
            expected_f,
            session.metrics.quality_scores.average_fidelity
        );
    }

    /// Edge case: Empty session should not panic
    #[test]
    fn test_update_metrics_empty_session() {
        let mut session = make_session();
        SessionQualityCollector::update_session_metrics(&mut session);
        assert_eq!(session.metrics.average_response_time_ms, 0.0);
    }

    // ─── generate_report tests ────────────────────────────────────────────────

    /// Req 34.5: Report generation with per-persona breakdown
    #[test]
    fn test_generate_report_per_persona_breakdown() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "q1", "r1", 1200, metrics_uncertain(),
        ));
        session.record_consultation(make_consultation(
            "mechanic", "q2", "r2", 800, metrics_confident(),
        ));
        session.record_consultation(make_consultation(
            "attendant", "q3", "r3", 600, metrics_redirect(),
        ));

        let report = SessionQualityCollector::generate_report(&session);

        assert_eq!(report.total_consultations, 3);
        assert!(report.per_persona_metrics.contains_key("mechanic"));
        assert!(report.per_persona_metrics.contains_key("attendant"));

        let mechanic = &report.per_persona_metrics["mechanic"];
        assert_eq!(mechanic.consultations, 2);
        assert!(
            (mechanic.average_response_time_ms - 1000.0).abs() < 0.01,
            "Mechanic avg time: (1200+800)/2 = 1000"
        );

        let attendant = &report.per_persona_metrics["attendant"];
        assert_eq!(attendant.consultations, 1);
        assert!(
            (attendant.average_response_time_ms - 600.0).abs() < 0.01,
            "Attendant avg time = 600"
        );
    }

    /// Req 34.5: Report tracks boundary violations per persona
    #[test]
    fn test_generate_report_tracks_boundary_violations() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "q1", "r1", 1000, metrics_boundary_violation(),
        ));
        session.record_consultation(make_consultation(
            "mechanic", "q2", "r2", 800, metrics_confident(),
        ));

        let report = SessionQualityCollector::generate_report(&session);
        let mechanic = &report.per_persona_metrics["mechanic"];
        assert_eq!(
            mechanic.boundary_violations, 1,
            "Should track 1 boundary violation for mechanic"
        );
    }

    /// Req 34.6: Report session-level rates are correct
    #[test]
    fn test_generate_report_session_level_rates() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "q1", "r1", 1200, metrics_uncertain(), // uncertainty: true
        ));
        session.record_consultation(make_consultation(
            "attendant", "q2", "r2", 600, metrics_redirect(), // redirection: true
        ));
        session.record_consultation(make_consultation(
            "mechanic", "q3", "r3", 800, metrics_confident(), // history_ref: true
        ));

        let report = SessionQualityCollector::generate_report(&session);

        assert!(
            (report.uncertainty_rate - 1.0 / 3.0).abs() < 0.01,
            "1/3 uncertain"
        );
        assert!(
            (report.redirection_rate - 1.0 / 3.0).abs() < 0.01,
            "1/3 redirection"
        );
        assert!(
            (report.history_reference_rate - 1.0 / 3.0).abs() < 0.01,
            "1/3 history reference"
        );
    }

    /// Req 34.6: Empty report for session with no consultations
    #[test]
    fn test_generate_report_empty_session() {
        let session = make_session();
        let report = SessionQualityCollector::generate_report(&session);
        assert_eq!(report.total_consultations, 0);
        assert!(report.per_persona_metrics.is_empty());
    }

    // ─── format_summary tests ─────────────────────────────────────────────────

    /// Req 34.6: Formatted summary contains key sections
    #[test]
    fn test_format_summary_contains_key_sections() {
        let mut session = make_session();
        session.record_consultation(make_consultation(
            "mechanic", "q1", "r1", 1200, metrics_uncertain(),
        ));
        let report = SessionQualityCollector::generate_report(&session);
        let summary = SessionQualityCollector::format_summary(&report);

        assert!(summary.contains("Quality Report"));
        assert!(summary.contains("Total consultations analyzed: 1"));
        assert!(summary.contains("hallucination score"));
        assert!(summary.contains("fidelity score"));
        assert!(summary.contains("Uncertainty rate"));
        assert!(summary.contains("Per-Persona Breakdown"));
        assert!(summary.contains("mechanic"));
    }
}
