//! Centralized Ollama configuration.
//!
//! All Ollama-specific configuration is defined and loaded here so model,
//! endpoint, and timeout policy do not spread through the application or
//! composition code.

use std::{env, fmt, time::Duration};

/// Configuration required by the Ollama inference adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaConfig {
    base_url: String,
    generation_model: String,
    timeout: Duration,
}

impl OllamaConfig {
    /// Loads Ollama configuration from the environment, using local defaults.
    pub fn from_env() -> Result<Self, OllamaConfigError> {
        let base_url =
            env::var("RECALL_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_owned());
        let generation_model =
            env::var("RECALL_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_owned());
        let timeout_secs = env::var("RECALL_OLLAMA_TIMEOUT_SECS")
            .unwrap_or_else(|_| "120".to_owned())
            .parse::<u64>()
            .map_err(|_| OllamaConfigError::InvalidTimeout)?;

        if base_url.trim().is_empty() {
            return Err(OllamaConfigError::EmptyBaseUrl);
        }
        if generation_model.trim().is_empty() {
            return Err(OllamaConfigError::EmptyModel);
        }

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            generation_model,
            timeout: Duration::from_secs(timeout_secs),
        })
    }

    /// Creates explicit configuration, primarily for tests and composition.
    pub fn new(
        base_url: String,
        generation_model: String,
        timeout: Duration,
    ) -> Result<Self, OllamaConfigError> {
        if base_url.trim().is_empty() {
            return Err(OllamaConfigError::EmptyBaseUrl);
        }
        if generation_model.trim().is_empty() {
            return Err(OllamaConfigError::EmptyModel);
        }
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            generation_model,
            timeout,
        })
    }

    /// Returns the configured Ollama base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns the configured generation model.
    pub fn generation_model(&self) -> &str {
        &self.generation_model
    }

    /// Returns the HTTP timeout.
    pub const fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Invalid centralized Ollama configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OllamaConfigError {
    /// The endpoint is empty.
    EmptyBaseUrl,
    /// The generation model is empty.
    EmptyModel,
    /// The configured timeout is not an unsigned number of seconds.
    InvalidTimeout,
}

impl fmt::Display for OllamaConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBaseUrl => formatter.write_str("Ollama URL cannot be empty"),
            Self::EmptyModel => formatter.write_str("Ollama generation model cannot be empty"),
            Self::InvalidTimeout => {
                formatter.write_str("RECALL_OLLAMA_TIMEOUT_SECS must be an integer")
            }
        }
    }
}

impl std::error::Error for OllamaConfigError {}
