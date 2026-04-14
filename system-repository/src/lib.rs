//! System Repository - Domain Expert CLI Engine
//!
//! This crate contains the execution engine: CLI, Ollama integration,
//! session management, prompt assembly, meeting orchestration,
//! quality assurance, and validation.
//!
//! It contains NO domain-specific content - all domain content
//! is loaded from a Context Repository at runtime.

pub mod cli;
pub mod ollama;
pub mod session;
pub mod config;
pub mod prompt;
pub mod meeting;
pub mod quality;
pub mod persona;
pub mod consultation;

#[cfg(test)]
mod property_tests;
