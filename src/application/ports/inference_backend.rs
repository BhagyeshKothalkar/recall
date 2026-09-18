//! Backend-neutral inference port.
//!
//! Concrete adapters such as Ollama, llama.cpp, or Candle implement this
//! capability. They must not query the memory store or determine provenance.

use std::fmt;

use crate::domain::{EmbeddingRequest, EmbeddingResponse, GenerationRequest, GenerationResponse};

/// Failure returned by an inference backend.
#[derive(Debug)]
pub enum InferenceError {
    /// The backend could not be reached or completed the requested operation.
    Backend(Box<dyn std::error::Error + Send + Sync>),
    /// The backend returned data that violated the domain response contract.
    InvalidResponse(String),
}

impl fmt::Display for InferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(error) => write!(formatter, "inference backend error: {error}"),
            Self::InvalidResponse(message) => {
                write!(formatter, "invalid inference response: {message}")
            }
        }
    }
}

impl std::error::Error for InferenceError {}

/// Capability boundary for generation and embedding.
///
/// The request types are domain-owned and backend-neutral. An implementation
/// translates them into its own transport or model-runtime representation.
pub trait InferenceBackend {
    /// Generates language from the question and explicitly supplied context.
    ///
    /// The backend must not perform additional memory retrieval.
    fn generate(&self, request: &GenerationRequest) -> Result<GenerationResponse, InferenceError>;

    /// Generates an embedding for one canonical memory.
    fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, InferenceError>;
}

impl<T: InferenceBackend + ?Sized> InferenceBackend for std::sync::Arc<T> {
    fn generate(&self, request: &GenerationRequest) -> Result<GenerationResponse, InferenceError> {
        (**self).generate(request)
    }

    fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, InferenceError> {
        (**self).embed(request)
    }
}
