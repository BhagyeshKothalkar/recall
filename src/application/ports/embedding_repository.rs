//! Port for durable derived embedding state.

use std::fmt;

use crate::domain::{EmbeddingResponse, InferenceModel, MemoryId};

/// Failure returned by an embedding repository.
#[derive(Debug)]
pub enum EmbeddingRepositoryError {
    /// The backing store could not complete the requested operation.
    Storage(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for EmbeddingRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "embedding repository storage error: {error}"),
        }
    }
}

impl std::error::Error for EmbeddingRepositoryError {}

/// Persistence port for derived embeddings.
pub trait EmbeddingRepository {
    /// Stores or replaces the derived embedding for a canonical memory.
    fn upsert(&self, embedding: &EmbeddingResponse) -> Result<(), EmbeddingRepositoryError>;

    /// Removes the derived embedding for a memory, if one exists.
    fn delete(&self, memory_id: MemoryId) -> Result<(), EmbeddingRepositoryError>;

    /// Returns whether a memory has a ready embedding for the selected model.
    fn exists_for_model(
        &self,
        memory_id: MemoryId,
        model: &InferenceModel,
    ) -> Result<bool, EmbeddingRepositoryError>;
}
