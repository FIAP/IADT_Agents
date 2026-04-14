//! CLI Engine module
//!
//! Handles user interaction, command parsing, routing, and display.

pub mod app;
pub mod commands;
pub mod engine;
pub mod theme;
pub mod tui;

pub use engine::CliEngine;
