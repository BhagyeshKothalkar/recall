//! Domain contracts for model inference.
//!
//! These are backend-neutral request and response values. Ollama, llama.cpp,
//! Candle, or another runtime must translate its own wire/API representation
//! into these types rather than leaking backend-specific structures inward.

use super::memory::MemoryId;
use super::search::RetrievedMemory;

/// Model identity used when requesting an embedding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InferenceModel(String);

impl InferenceModel {
    /// Creates a named model identifier.
    pub fn new(name: String) -> Option<Self> {
        if name.trim().is_empty() {
            None
        } else {
            Some(Self(name))
        }
    }

    /// Returns the configured model name.
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// Input to text generation.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerationRequest {
    question: String,
    context: Vec<RetrievedMemory>,
}

impl GenerationRequest {
    /// Creates a generation request with explicitly selected memory context.
    pub fn new(question: String, context: Vec<RetrievedMemory>) -> Option<Self> {
        if question.trim().is_empty() {
            None
        } else {
            Some(Self { question, context })
        }
    }

    /// Returns the user's question.
    pub fn question(&self) -> &str {
        &self.question
    }

    /// Returns the exact memory context selected by Recall.
    pub fn context(&self) -> &[RetrievedMemory] {
        &self.context
    }

    /// Returns the memory identities supplied as generation context.
    pub fn source_ids(&self) -> Vec<MemoryId> {
        self.context.iter().map(RetrievedMemory::memory_id).collect()
    }
}

/// Output produced by a generation backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationResponse {
    text: String,
}

impl GenerationResponse {
    /// Creates a generation response after rejecting empty output.
    pub fn new(text: String) -> Option<Self> {
        if text.trim().is_empty() {
            None
        } else {
            Some(Self { text })
        }
    }

    /// Returns generated text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Input to an embedding backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddingRequest {
    memory_id: MemoryId,
    text: String,
    model: InferenceModel,
}

impl EmbeddingRequest {
    /// Creates an embedding request for one canonical memory.
    pub fn new(memory_id: MemoryId, text: String, model: InferenceModel) -> Option<Self> {
        if text.trim().is_empty() {
            None
        } else {
            Some(Self {
                memory_id,
                text,
                model,
            })
        }
    }

    /// Returns the memory being embedded.
    pub const fn memory_id(&self) -> MemoryId {
        self.memory_id
    }

    /// Returns the canonical text being embedded.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the model selected for embedding.
    pub fn model(&self) -> &InferenceModel {
        &self.model
    }
}

/// Output produced by an embedding backend.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddingResponse {
    memory_id: MemoryId,
    model: InferenceModel,
    vector: Vec<f32>,
}

impl EmbeddingResponse {
    /// Creates an embedding response, rejecting an empty vector.
    pub fn new(memory_id: MemoryId, model: InferenceModel, vector: Vec<f32>) -> Option<Self> {
        if vector.is_empty() || vector.iter().any(|value| value.is_nan()) {
            None
        } else {
            Some(Self {
                memory_id,
                model,
                vector,
            })
        }
    }

    /// Returns the memory associated with the embedding.
    pub const fn memory_id(&self) -> MemoryId {
        self.memory_id
    }

    /// Returns the embedding model identity.
    pub fn model(&self) -> &InferenceModel {
        &self.model
    }

    /// Returns the embedding vector.
    pub fn vector(&self) -> &[f32] {
        &self.vector
    }
}
