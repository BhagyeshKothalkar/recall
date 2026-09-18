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
    embedding_model: String,
    timeout: Duration,
}

impl OllamaConfig {
    /// Loads Ollama configuration from the environment, using local defaults.
    pub fn from_env() -> Result<Self, OllamaConfigError> {
        let base_url =
            env::var("RECALL_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_owned());
        let generation_model =
            env::var("RECALL_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_owned());
        let embedding_model =
            env::var("RECALL_OLLAMA_EMBEDDING_MODEL").unwrap_or_else(|_| generation_model.clone());
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
        if embedding_model.trim().is_empty() {
            return Err(OllamaConfigError::EmptyEmbeddingModel);
        }

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            generation_model,
            embedding_model,
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
        Self::new_with_embedding_model(
            base_url,
            generation_model.clone(),
            generation_model,
            timeout,
        )
    }

    /// Creates explicit configuration with separate generation and embedding models.
    pub fn new_with_embedding_model(
        base_url: String,
        generation_model: String,
        embedding_model: String,
        timeout: Duration,
    ) -> Result<Self, OllamaConfigError> {
        if base_url.trim().is_empty() {
            return Err(OllamaConfigError::EmptyBaseUrl);
        }
        if generation_model.trim().is_empty() {
            return Err(OllamaConfigError::EmptyModel);
        }
        if embedding_model.trim().is_empty() {
            return Err(OllamaConfigError::EmptyEmbeddingModel);
        }
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            generation_model,
            embedding_model,
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

    /// Returns the configured embedding model.
    pub fn embedding_model(&self) -> &str {
        &self.embedding_model
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
    /// The embedding model is empty.
    EmptyEmbeddingModel,
    /// The configured timeout is not an unsigned number of seconds.
    InvalidTimeout,
}

impl fmt::Display for OllamaConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBaseUrl => formatter.write_str("Ollama URL cannot be empty"),
            Self::EmptyModel => formatter.write_str("Ollama generation model cannot be empty"),
            Self::EmptyEmbeddingModel => {
                formatter.write_str("Ollama embedding model cannot be empty")
            }
            Self::InvalidTimeout => {
                formatter.write_str("RECALL_OLLAMA_TIMEOUT_SECS must be an integer")
            }
        }
    }
}

impl std::error::Error for OllamaConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_configuration_defaults_embedding_model_to_generation_model() {
        let config = OllamaConfig::new(
            "http://localhost:11434/".to_owned(),
            "llama3.2".to_owned(),
            Duration::from_secs(10),
        )
        .unwrap();

        assert_eq!(config.base_url(), "http://localhost:11434");
        assert_eq!(config.embedding_model(), "llama3.2");
    }

    #[test]
    fn explicit_configuration_accepts_separate_embedding_model() {
        let config = OllamaConfig::new_with_embedding_model(
            "http://localhost:11434".to_owned(),
            "llama3.2".to_owned(),
            "nomic-embed-text".to_owned(),
            Duration::from_secs(10),
        )
        .unwrap();

        assert_eq!(config.generation_model(), "llama3.2");
        assert_eq!(config.embedding_model(), "nomic-embed-text");
    }

    #[test]
    fn empty_embedding_model_is_rejected() {
        let error = OllamaConfig::new_with_embedding_model(
            "http://localhost:11434".to_owned(),
            "llama3.2".to_owned(),
            "  ".to_owned(),
            Duration::from_secs(10),
        )
        .unwrap_err();

        assert_eq!(error, OllamaConfigError::EmptyEmbeddingModel);
    }
}
