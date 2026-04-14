//! Domain Expert CLI System - Main Entry Point
//!
//! Educational platform that enables students to interact with simulated
//! domain experts powered by local LLM models through Ollama.

use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

/// Domain Expert CLI System
#[derive(Parser, Debug)]
#[command(name = "domain-expert")]
#[command(about = "Educational CLI for domain expert simulation using local LLMs")]
#[command(version)]
struct Cli {
    /// Path to the context repository
    #[arg(short, long, default_value = "./context")]
    context: PathBuf,

    /// Scenario to load
    #[arg(short, long)]
    scenario: Option<String>,

    /// Resume a previous session
    #[arg(long)]
    session_id: Option<String>,

    /// Ollama endpoint URL
    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,

    /// Ollama model override
    #[arg(long)]
    model: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&cli.log_level)),
        )
        .with_target(false)
        .with_thread_ids(false)
        .init();

    tracing::info!("Domain Expert CLI System starting...");
    tracing::info!("Context repository: {}", cli.context.display());

    // Load and validate context repository
    let context = match context_repository::loader::load_context_repository(&cli.context) {
        Ok(ctx) => {
            tracing::info!(
                "Loaded context: {} ({})",
                ctx.contract.domain_name,
                ctx.contract.domain_id
            );
            tracing::info!("Contract version: {}", ctx.contract.contract_version);
            tracing::info!("Personas: {}", ctx.personas.len());
            tracing::info!("Scenarios: {}", ctx.scenarios.len());
            ctx
        }
        Err(e) => {
            tracing::error!("Failed to load context repository: {}", e);
            eprintln!("\n❌ Error loading context repository:");
            eprintln!("   {}", e);
            eprintln!("\n💡 Troubleshooting:");
            eprintln!("   1. Verify the path: {}", cli.context.display());
            eprintln!("   2. Run: domain-expert validate-contract {}", cli.context.display());
            eprintln!("   3. Check that all required files exist");
            std::process::exit(1);
        }
    };

    // Deep validation
    let validation = context_repository::validation::validate_loaded_context(&context);
    if !validation.valid {
        tracing::error!("Context validation failed");
        eprintln!("\n❌ Context validation errors:");
        for error in &validation.errors {
            eprintln!("   • {} - {}", error.file, error.message);
            if let Some(suggestion) = &error.suggestion {
                eprintln!("     💡 {}", suggestion);
            }
        }
        std::process::exit(1);
    }
    for warning in &validation.warnings {
        tracing::warn!("{}", warning);
    }

    // Initialize system configuration
    let system_config = system_repository::config::SystemConfig::load_with_overrides(
        cli.ollama_url.clone(),
        cli.model.clone(),
    );

    // Determine scenario
    let scenario = if let Some(scenario_id) = &cli.scenario {
        context
            .scenarios
            .iter()
            .find(|s| s.scenario_id == *scenario_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Scenario '{}' not found. Available: {:?}",
                    scenario_id,
                    context
                        .scenarios
                        .iter()
                        .map(|s| &s.scenario_id)
                        .collect::<Vec<_>>()
                )
            })?
            .clone()
    } else {
        // Use first scenario as default
        context.scenarios.first()
            .ok_or_else(|| anyhow::anyhow!("No scenarios available in context repository"))?
            .clone()
    };

    tracing::info!("Scenario: {} - {}", scenario.scenario_id, scenario.name);

    // Create or restore session
    let session = if let Some(session_id) = &cli.session_id {
        tracing::info!("Restoring session: {}", session_id);
        system_repository::session::SessionManager::restore_session(
            session_id,
            &system_config.session.storage_location,
        )
        .await?
    } else {
        context_repository::Session::new(
            &context.contract.domain_id,
            &context.contract.contract_version,
            &scenario.scenario_id,
        )
    };

    // Start CLI engine
    let mut engine = system_repository::cli::CliEngine::new(
        context,
        session,
        scenario,
        system_config,
    );

    engine.run().await?;

    Ok(())
}
