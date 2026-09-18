//! Ollama HTTP inference adapter.
//!
//! The adapter translates backend-neutral generation requests into Ollama's
//! `/api/generate` request and maps the response back to the domain contract.

use std::fmt;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::application::ports::{InferenceBackend, InferenceError, TextEmbedder};
use crate::domain::{
    EmbeddingRequest, EmbeddingResponse, GenerationRequest, GenerationResponse, InferenceModel,
};

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
            .field("embedding_model", &self.config.embedding_model())
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

        GenerationResponse::new(response.response).ok_or_else(|| {
            InferenceError::InvalidResponse("Ollama returned empty generation".to_owned())
        })
    }

    fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, InferenceError> {
        let vector = self.embed_vector(request.text(), request.model().name())?;

        EmbeddingResponse::new(request.memory_id(), request.model().clone(), vector).ok_or_else(
            || InferenceError::InvalidResponse("Ollama returned an invalid embedding".to_owned()),
        )
    }
}

impl TextEmbedder for OllamaBackend {
    fn embed_text(&self, text: &str, model: &InferenceModel) -> Result<Vec<f32>, InferenceError> {
        self.embed_vector(text, model.name())
    }
}

impl OllamaBackend {
    fn embed_vector(&self, text: &str, model: &str) -> Result<Vec<f32>, InferenceError> {
        if text.trim().is_empty() {
            return Err(InferenceError::InvalidResponse(
                "embedding input cannot be empty".to_owned(),
            ));
        }

        let payload = EmbedRequest {
            model: model.to_owned(),
            input: text.to_owned(),
        };

        let response = self
            .client
            .post(format!("{}/api/embed", self.config.base_url()))
            .json(&payload)
            .send()
            .map_err(backend_error)?
            .error_for_status()
            .map_err(backend_error)?
            .json::<EmbedResponse>()
            .map_err(backend_error)?;

        validate_embedding(response)
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

#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    input: String,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

fn validate_embedding(response: EmbedResponse) -> Result<Vec<f32>, InferenceError> {
    if response.embeddings.len() != 1 {
        return Err(InferenceError::InvalidResponse(format!(
            "Ollama returned {} embeddings for one input",
            response.embeddings.len()
        )));
    }

    let vector = response.embeddings.into_iter().next().ok_or_else(|| {
        InferenceError::InvalidResponse("Ollama returned no embedding".to_owned())
    })?;
    if vector.is_empty() || vector.iter().any(|value| !value.is_finite()) {
        return Err(InferenceError::InvalidResponse(
            "Ollama returned an empty or non-finite embedding".to_owned(),
        ));
    }

    Ok(vector)
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
            Self::Client(error) => {
                write!(formatter, "could not create Ollama HTTP client: {error}")
            }
        }
    }
}

impl std::error::Error for OllamaError {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn embed_request_serializes_ollama_api_shape() {
        let payload = EmbedRequest {
            model: "embed-model".to_owned(),
            input: "remember this".to_owned(),
        };
        let json = serde_json::to_value(payload).unwrap();

        assert_eq!(json["model"], "embed-model");
        assert_eq!(json["input"], "remember this");
    }

    #[test]
    fn text_embedder_rejects_empty_input_before_http() {
        let config = OllamaConfig::new(
            "http://127.0.0.1:1".to_owned(),
            "model".to_owned(),
            Duration::from_secs(1),
        )
        .unwrap();
        let backend = OllamaBackend::new(config).unwrap();
        let model = InferenceModel::new("model".to_owned()).unwrap();

        let error = TextEmbedder::embed_text(&backend, "  ", &model).unwrap_err();

        assert!(
            matches!(error, InferenceError::InvalidResponse(message) if message.contains("empty"))
        );
    }

    #[test]
    fn invalid_vectors_are_rejected() {
        for response in [
            EmbedResponse {
                embeddings: vec![vec![]],
            },
            EmbedResponse { embeddings: vec![] },
            EmbedResponse {
                embeddings: vec![vec![1.0], vec![2.0]],
            },
            EmbedResponse {
                embeddings: vec![vec![f32::INFINITY]],
            },
        ] {
            assert!(matches!(
                validate_embedding(response),
                Err(InferenceError::InvalidResponse(_))
            ));
        }
    }
}
