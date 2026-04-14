//! CLI Engine - Main interactive loop with Ratatui TUI
//!
//! Orchestrates user interaction, command routing, and display.

use context_repository::loader::LoadedContext;
use context_repository::models::scenario::ScenarioDefinition;
use context_repository::models::session::{Decision, Session};

use crate::cli::app::{App, AppMode};
use crate::cli::commands::{self, Command};
use crate::cli::tui::Tui;
use crate::config::SystemConfig;
use crate::consultation::ConsultationOrchestrator;
use crate::meeting::{ConclusionReason, MeetingConfig, MeetingOrchestrator};
use crate::ollama::OllamaClient;
use crate::prompt::PromptAssembler;
use crate::quality::collector::SessionQualityCollector;
use crate::quality::consistency::ConsistencyValidator;
use crate::quality::hallucination::HallucinationDetector;
use crate::quality::ManipulationDetector;
use crate::session::SessionManager;

use crossterm::event::{Event, KeyCode};
use tokio_stream::StreamExt;
use tui_input::backend::crossterm::EventHandler;

/// The main CLI engine that drives user interaction
pub struct CliEngine {
    context: LoadedContext,
    session: Session,
    scenario: ScenarioDefinition,
    config: SystemConfig,
    ollama: OllamaClient,
    prompt_assembler: PromptAssembler,
    session_manager: SessionManager,
    manipulation_detector: ManipulationDetector,
    active_persona: Option<String>,
}

impl CliEngine {
    pub fn new(
        context: LoadedContext,
        session: Session,
        scenario: ScenarioDefinition,
        config: SystemConfig,
    ) -> Self {
        let ollama = OllamaClient::new(
            &config.ollama.endpoint,
            config.ollama.timeout_secs,
            config.ollama.retry_attempts,
        );

        let manipulation_detector = ManipulationDetector::with_custom(
            &context.config.manipulation_detection.patterns,
        );

        let prompt_assembler = PromptAssembler::new();
        let session_manager = SessionManager::new(&config.session.storage_location);

        Self {
            context,
            session,
            scenario,
            config,
            ollama,
            prompt_assembler,
            session_manager,
            manipulation_detector,
            active_persona: None,
        }
    }

    /// Run the interactive TUI loop
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut tui = Tui::new()?;
        let mut app = App::new();

        self.display_welcome(&mut app);

        let mut event_reader = crossterm::event::EventStream::new();

        loop {
            // Update active persona name for status bar
            app.active_persona_name = self.active_persona.clone().map(|id| {
                self.context.personas.iter().find(|p| p.persona_id == id)
                    .map(|p| format!("{} ({})", p.name, p.role))
                    .unwrap_or(id)
            });

            tui.draw(&mut app)?;

            tokio::select! {
                Some(Ok(event)) = event_reader.next() => {
                    match event {
                        Event::Key(key) => {
                            match key.code {
                                KeyCode::Esc => {
                                    break;
                                }
                                KeyCode::PageUp => app.scroll_up(),
                                KeyCode::PageDown => app.scroll_down(),
                                KeyCode::Enter => {
                                    let input = app.input.value().trim().to_string();
                                    app.input.reset();

                                    if input.is_empty() {
                                        continue;
                                    }

                                    app.add_message("User", &input, false, false);
                                    app.is_loading = true;
                                    tui.draw(&mut app)?; // force draw loading state!

                                    // Parse and handle
                                    match commands::parse_command(&input) {
                                        Ok(cmd) => {
                                            if let Command::Exit = cmd {
                                                break;
                                            }
                                            if let Err(e) = self.handle_command(&mut app, cmd).await {
                                                app.add_message("Sistema", &format!("Erro: {}", e), true, true);
                                            }
                                        }
                                        Err(e) => {
                                            app.add_message("Sistema", &format!("Erro: {}\n{}", e, commands::help_text()), true, true);
                                        }
                                    }
                                    app.is_loading = false;
                                }
                                _ => {
                                    app.input.handle_event(&Event::Key(key));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Restore terminal
        tui.restore()?;
        self.save_session().await?;
        println!("\n👋 O Sistema FIAP + Alura foi encerrado com sucesso. Sessão salva.\n");

        Ok(())
    }

    async fn handle_command(&mut self, app: &mut App, cmd: Command) -> anyhow::Result<()> {
        match cmd {
            Command::Persona { name } => self.handle_persona(app, &name).await,
            Command::Reuniao { personas, topic } => self.handle_reuniao(app, &personas, &topic).await,
            Command::Decisao { description } => self.handle_decisao(app, &description),
            Command::Historico => self.handle_historico(app),
            Command::Contexto => self.handle_contexto(app),
            Command::Qualidade => self.handle_qualidade(app),
            Command::Consistencia { persona_id } => self.handle_consistencia(app, persona_id.as_deref()),
            Command::Help => {
                app.add_message("Ajuda", &commands::help_text(), true, false);
                Ok(())
            }
            Command::Message { content } => self.handle_message(app, &content).await,
            Command::Exit => Ok(()), // Handled in loop
        }
    }

    async fn handle_persona(&mut self, app: &mut App, name: &str) -> anyhow::Result<()> {
        let persona = self
            .context
            .personas
            .iter()
            .find(|p| p.persona_id == name || p.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| {
                let available: Vec<String> = self
                    .context
                    .personas
                    .iter()
                    .map(|p| format!("{} ({})", p.persona_id, p.name))
                    .collect();
                anyhow::anyhow!(
                    "Especialista '{}' não encontrado.\nDisponíveis:\n  {}",
                    name,
                    available.join("\n  ")
                )
            })?;

        if !self.scenario.available_experts.contains(&persona.persona_id) {
            return Err(anyhow::anyhow!(
                "Expert '{}' não está disponível no cenário '{}'",
                persona.name,
                self.scenario.name
            ));
        }

        let mut msg = String::new();
        msg.push_str(&format!("👤 {} - {}\n", persona.name, persona.role));
        msg.push_str(&format!("Estilo: {}\n\n", persona.behavioral_patterns.communication_style));
        msg.push_str("Conexão estabelecida com sucesso.");

        app.add_message("Sistema", &msg, true, false);
        self.active_persona = Some(persona.persona_id.clone());

        Ok(())
    }

    async fn handle_message(&mut self, app: &mut App, content: &str) -> anyhow::Result<()> {
        let persona_query = match &self.active_persona {
            Some(id) => id.clone(),
            None => {
                let msg = format!("💡 Nenhum especialista ativo. Use /persona <nome> primeiro.\nDisponíveis: {}", self.scenario.available_experts.join(", "));
                app.add_message("Sistema", &msg, true, true);
                return Ok(());
            }
        };

        let effective_model = if self.context.config.ollama_model.is_empty() {
            self.config.ollama.default_model.clone()
        } else {
            self.context.config.ollama_model.clone()
        };

        let persona_name = self
            .context
            .personas
            .iter()
            .find(|p| p.persona_id == persona_query || p.name.to_lowercase().contains(&persona_query.to_lowercase()))
            .map(|p| format!("{} ({})", p.name, p.role))
            .unwrap_or_else(|| persona_query.clone());

        let orchestrator = ConsultationOrchestrator::new(
            &self.context,
            &self.scenario,
            &self.ollama,
            &self.prompt_assembler,
            &self.manipulation_detector,
            &effective_model,
        );

        match orchestrator.consult(&self.session, &persona_query, content).await {
            Ok(result) => {
                if result.manipulation_detected {
                    self.session.metrics.manipulation_attempts += 1;
                }

                let mut indicators = Vec::new();
                if let Some(ref metrics) = result.consultation.quality_metrics {
                    if metrics.uncertainty_expressed { indicators.push("⚠️ incerteza expressa"); }
                    if metrics.redirected_appropriately { indicators.push("↗️ redirecionamento"); }
                    if metrics.history_referenced { indicators.push("📋 referenciou histórico"); }
                    if !metrics.knowledge_boundary_respected { indicators.push("🚨 possível quebra de limite"); }
                    indicators.push(Box::leak(format!("{}ms", metrics.response_time_ms).into_boxed_str()));
                }

                let mut final_message = result.consultation.expert_response.clone();
                if !indicators.is_empty() {
                    final_message.push_str(&format!("\n\n[ {} ]", indicators.join(" | ")));
                }

                app.add_message(&persona_name, &final_message, false, false);
                self.session.record_consultation(result.consultation);

                if let Err(e) = self.session_manager.persist_session(&self.session).await {
                    tracing::warn!("Auto-save failed: {}", e);
                }
            }
            Err(e) => {
                app.add_message("Sistema", &format!("Falha na conexão neural: {}", e), true, true);
                // Clear active persona on hard fail
                if let crate::consultation::ConsultationError::PersonaNotFound(_,_) | crate::consultation::ConsultationError::PersonaNotInScenario{..} = e {
                    self.active_persona = None;
                }
            }
        }
        Ok(())
    }

    async fn handle_reuniao(&mut self, app: &mut App, persona_ids: &[String], topic: &str) -> anyhow::Result<()> {
        for name in persona_ids {
            if !self.context.personas.iter().any(|p| p.persona_id == *name) {
                return Err(anyhow::anyhow!("Especialista '{}' não encontrado", name));
            }
            if !self.scenario.available_experts.contains(name) {
                return Err(anyhow::anyhow!("Especialista '{}' indisponível no cenário atual", name));
            }
        }

        let participants: Vec<&context_repository::models::persona::PersonaDefinition> = persona_ids
            .iter()
            .filter_map(|id| self.context.personas.iter().find(|p| p.persona_id == *id))
            .collect();

        let mut header = format!("📝 Iniciando Reunião: {}\nParticipantes:\n", topic);
        for p in &participants {
            header.push_str(&format!("  • {} - {}\n", p.name, p.role));
        }
        app.add_message("Reunião", &header, true, false);
        app.is_loading = true; // Let the async loop know we enter heavy work

        let mut meeting = MeetingOrchestrator::create_meeting(&participants, topic);

        let config = MeetingConfig {
            turn_limit: self.context.config.meeting_turn_limit,
            min_turns_before_conclusion: 4,
        };

        let effective_model = if self.context.config.ollama_model.is_empty() {
            self.config.ollama.default_model.clone()
        } else {
            self.context.config.ollama_model.clone()
        };

        let orchestrator = MeetingOrchestrator::new(
            &self.context,
            &self.ollama,
            &self.prompt_assembler,
            &self.manipulation_detector,
            &effective_model,
            config.clone(),
        );

        let start = std::time::Instant::now();
        let mut turn_number: u32 = 1;

        loop {
            if let Some(reason) = orchestrator.should_conclude(&meeting, turn_number) {
                let msg = match reason {
                    ConclusionReason::TurnLimitReached => format!("Limite de turnos ({}) atingido.", config.turn_limit),
                    ConclusionReason::NaturalConclusion => "Participantes chegaram a uma conclusão natural.".to_string(),
                };
                meeting.conclusion = Some(msg);
                break;
            }

            let speaker = participants[((turn_number - 1) as usize) % participants.len()];

            match orchestrator.process_turn(&meeting, speaker, &self.session, turn_number).await {
                Ok(result) => {
                    app.add_message(&result.persona_name, &result.turn.statement, false, false);
                    meeting.turns.push(result.turn);
                }
                Err(e) => {
                    app.add_message("Sistema", &format!("Erro no turno de {}: {}", speaker.name, e), true, true);
                }
            }
            turn_number += 1;
        }

        meeting.duration_ms = start.elapsed().as_millis() as u64;
        meeting.conflicts = MeetingOrchestrator::detect_conflicts(&meeting.turns, &participants);

        let summary = MeetingOrchestrator::generate_summary(&meeting);
        app.add_message("Resumo da Reunião", &summary, true, false);

        self.session.record_meeting(meeting);
        let _ = self.session_manager.persist_session(&self.session).await;

        Ok(())
    }

    fn handle_decisao(&mut self, app: &mut App, description: &str) -> anyhow::Result<()> {
        let mut decision = Decision::new(description);
        let recent: Vec<String> = self.session.consultation_history.iter().rev().take(5).map(|c| c.consultation_id.clone()).collect();
        decision.prior_consultations = recent;
        decision.scenario_state = self.session.state.current_scenario_state.clone();

        self.session.record_decision(decision);

        app.add_message("Sistema", &format!("✅ Decisão registrada com sucesso.\nTotal de decisões: {}", self.session.decision_history.len()), true, false);
        Ok(())
    }

    fn handle_historico(&self, app: &mut App) -> anyhow::Result<()> {
        if self.session.decision_history.is_empty() {
            app.add_message("Histórico", "Nenhuma decisão registrada ainda.", true, false);
            return Ok(());
        }

        let mut out = String::new();
        for (i, d) in self.session.decision_history.iter().enumerate() {
            out.push_str(&format!("{}. [{}] {}\n", i + 1, d.timestamp.format("%H:%M:%S"), d.description));
            if !d.reasoning.is_empty() { out.push_str(&format!("   Justificativa: {}\n", d.reasoning)); }
        }
        app.add_message("Histórico de Decisões", &out, true, false);
        Ok(())
    }

    fn handle_contexto(&self, app: &mut App) -> anyhow::Result<()> {
        let mut out = format!("📌 Cenário: {}\n{}\n\n🎯 Objetivos:\n", self.scenario.name, self.scenario.description);
        for obj in &self.scenario.learning_objectives { out.push_str(&format!("  • {}\n", obj)); }
        
        out.push_str("\n👥 Especialistas Disponíveis:\n");
        for id in &self.scenario.available_experts {
            if let Some(p) = self.context.personas.iter().find(|p| p.persona_id == *id) {
                out.push_str(&format!("  • {} - {}\n", p.name, p.role));
            }
        }
        app.add_message("Contexto Global", &out, true, false);
        Ok(())
    }

    fn handle_qualidade(&self, app: &mut App) -> anyhow::Result<()> {
        if self.session.consultation_history.is_empty() {
            app.add_message("Qualidade", "Nenhuma métrica disponível. Faça consultas primeiro.", true, true);
            return Ok(());
        }

        let mut metrics_session = self.session.clone();
        SessionQualityCollector::update_session_metrics(&mut metrics_session);
        let report = SessionQualityCollector::generate_report(&metrics_session);
        let mut summary = SessionQualityCollector::format_summary(&report);

        if let Some(last) = self.session.consultation_history.last() {
            if let Some(persona) = self.context.personas.iter().find(|p| p.persona_id == last.persona_id) {
                let h_analysis = HallucinationDetector::analyze(&last.expert_response, persona, &self.context.world_model);
                if h_analysis.score > 0.0 {
                    summary.push_str(&format!("\n\n─── Risco de Alucinação (Última) ───\nScore: {:.0}%\n", h_analysis.score * 100.0));
                    for ind in &h_analysis.indicators {
                        summary.push_str(&format!("  • {:?}: {}\n", ind.kind, ind.evidence));
                    }
                }
            }
        }

        app.add_message("Relatório de Qualidade", &summary, true, false);
        Ok(())
    }

    fn handle_consistencia(&self, app: &mut App, persona_id: Option<&str>) -> anyhow::Result<()> {
        if self.session.consultation_history.is_empty() {
            app.add_message("Consistência", "Nenhuma atuação para analisar.", true, true);
            return Ok(());
        }

        let target_personas: Vec<_> = if let Some(id) = persona_id {
            vec![self.context.personas.iter().find(|p| p.persona_id == id).ok_or_else(|| anyhow::anyhow!("Especialista não encontrado"))?]
        } else {
            let consulted_ids: std::collections::HashSet<&str> = self.session.consultation_history.iter().map(|c| c.persona_id.as_str()).collect();
            self.context.personas.iter().filter(|p| consulted_ids.contains(p.persona_id.as_str())).collect()
        };

        for persona in target_personas {
            let report = ConsistencyValidator::validate(&self.session, persona);
            app.add_message(&format!("Consistência ({})", persona.name), &ConsistencyValidator::format_report(&report), true, false);
        }
        Ok(())
    }

    fn display_welcome(&self, app: &mut App) {
        let msg = format!(
            "Sessão: {}\nDomínio: {}\nCenário: {}\n\nDigite /ajuda para comandos.",
            &self.session.session_id[..8],
            self.context.contract.domain_name,
            self.scenario.name,
        );
        app.add_message("Conexão Estabelecida", &msg, true, false);
    }

    async fn save_session(&self) -> anyhow::Result<()> {
        self.session_manager.persist_session(&self.session).await
    }
}
