//! Context Repository - Domain models and contract specification
//!
//! This crate defines the data structures and contract interface between
//! the System Repository (engine) and domain-specific content.
//! It contains NO execution logic, only data models and validation.

pub mod contract;
pub mod models;
pub mod loader;
pub mod validation;

pub use contract::*;
pub use models::*;

