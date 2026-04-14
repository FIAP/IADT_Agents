//! Session Manager module
//!
//! Handles session creation, persistence, restoration, and summary generation.

use std::path::{Path, PathBuf};

use context_repository::models::session::Session;
use thiserror::Error;

/// Errors during session operations
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Failed to persist session: {0}")]
    PersistenceError(String),

    #[error("Failed to restore session: {0}")]
    RestoreError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Manages session lifecycle and persistence
pub struct SessionManager {
    storage_path: PathBuf,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(storage_path: &str) -> Self {
        Self {
            storage_path: PathBuf::from(storage_path),
        }
    }

    /// Persist a session to disk
    pub async fn persist_session(&self, session: &Session) -> anyhow::Result<()> {
        // Ensure storage directory exists
        tokio::fs::create_dir_all(&self.storage_path).await?;

        let file_path = self
            .storage_path
            .join(format!("{}.json", session.session_id));

        let json = serde_json::to_string_pretty(session)?;

        // Atomic write: write to temp file then rename
        let temp_path = file_path.with_extension("tmp");
        tokio::fs::write(&temp_path, &json).await?;
        tokio::fs::rename(&temp_path, &file_path).await?;

        tracing::debug!("Session persisted: {}", file_path.display());

        Ok(())
    }

    /// Restore a session from disk
    pub async fn restore_session(
        session_id: &str,
        storage_path: &str,
    ) -> anyhow::Result<Session> {
        let file_path = Path::new(storage_path).join(format!("{}.json", session_id));

        if !file_path.exists() {
            return Err(anyhow::anyhow!("Session file not found: {}", file_path.display()));
        }

        let json = tokio::fs::read_to_string(&file_path).await?;
        let session: Session = serde_json::from_str(&json)?;

        tracing::info!("Session restored: {}", session_id);

        Ok(session)
    }

    /// List all available sessions
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
        let mut sessions = Vec::new();

        if !self.storage_path.exists() {
            return Ok(sessions);
        }

        let mut entries = tokio::fs::read_dir(&self.storage_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    sessions.push(name.to_string());
                }
            }
        }

        Ok(sessions)
    }

    /// Generate a summary of a session
    pub fn generate_summary(session: &Session) -> String {
        let mut summary = String::new();

        summary.push_str(&format!(
            "Session Summary: {}\n",
            &session.session_id[..8]
        ));
        summary.push_str(&format!(
            "Domain: {}\n",
            session.context_repository_id
        ));
        summary.push_str(&format!(
            "Scenario: {}\n",
            session.scenario_id
        ));
        summary.push_str(&format!(
            "Duration: {} to {}\n",
            session.created_at.format("%Y-%m-%d %H:%M"),
            session.last_accessed_at.format("%Y-%m-%d %H:%M")
        ));
        summary.push_str(&format!(
            "Decisions: {}\n",
            session.decision_history.len()
        ));
        summary.push_str(&format!(
            "Consultations: {}\n",
            session.consultation_history.len()
        ));
        summary.push_str(&format!(
            "Meetings: {}\n",
            session.meeting_history.len()
        ));

        if !session.decision_history.is_empty() {
            summary.push_str("\nDecisions:\n");
            for (i, decision) in session.decision_history.iter().enumerate() {
                summary.push_str(&format!(
                    "  {}. [{}] {}\n",
                    i + 1,
                    decision.timestamp.format("%H:%M:%S"),
                    decision.description
                ));
            }
        }

        summary
    }
}
