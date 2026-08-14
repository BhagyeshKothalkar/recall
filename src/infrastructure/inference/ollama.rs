//! Ollama HTTP inference adapter.
//!
//! The adapter translates backend-neutral generation requests into Ollama's
//! `/api/generate` request and maps the response back to the domain contract.

use std::fmt;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::application::ports::{InferenceBackend, InferenceError};
use crate::domain::{EmbeddingRequest, EmbeddingResponse, GenerationRequest, GenerationResponse};

use super::{config::OllamaConfig, prompt};

/// Concrete inference backend backed by a local or explicitly configured Ollama server.
pub struct OllamaBackend {
    client: Client,
    config: OllamaConfig,
}

impl OllamaBackend {
    /// Creates an Ollama adapter from centralized configuration.
    pub fn new(config: OllamaConfig) -> Result<Self, OllamaError> {
        let client = Client::builder()
            .timeout(config.timeout())
            .build()
            .map_err(OllamaError::Client)?;
        Ok(Self { client, config })
    }
}

impl fmt::Debug for OllamaBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OllamaBackend")
            .field("base_url", &self.config.base_url())
            .field("generation_model", &self.config.generation_model())
            .finish_non_exhaustive()
    }
}

impl InferenceBackend for OllamaBackend {
    fn generate(&self, request: &GenerationRequest) -> Result<GenerationResponse, InferenceError> {
        let built = prompt::build(request);
        let payload = GenerateRequest {
            model: self.config.generation_model().to_owned(),
            prompt: built.prompt,
            system: built.system,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.config.base_url()))
            .json(&payload)
            .send()
            .map_err(backend_error)?;

        let response = response
            .error_for_status()
            .map_err(backend_error)?
            .json::<GenerateResponse>()
            .map_err(backend_error)?;

        GenerationResponse::new(response.response)
            .ok_or_else(|| InferenceError::InvalidResponse("Ollama returned empty generation".to_owned()))
    }

    fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, InferenceError> {
        Err(InferenceError::InvalidResponse(format!(
            "embedding is not implemented by the Ollama generation slice for model {}",
            request.model().name()
        )))
    }
}

#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    system: String,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
}

fn backend_error(error: reqwest::Error) -> InferenceError {
    InferenceError::Backend(Box::new(error))
}

/// Adapter construction failure.
#[derive(Debug)]
pub enum OllamaError {
    /// The HTTP client could not be constructed.
    Client(reqwest::Error),
}

impl fmt::Display for OllamaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client(error) => write!(formatter, "could not create Ollama HTTP client: {error}"),
        }
    }
}

impl std::error::Error for OllamaError {}
