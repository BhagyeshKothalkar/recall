//! Concrete inference adapters and their centralized configuration.

pub mod config;
pub mod ollama;
mod prompt;

pub use config::{OllamaConfig, OllamaConfigError};
pub use ollama::{OllamaBackend, OllamaError};
