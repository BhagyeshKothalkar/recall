//! Port for canonical memory persistence.
//!
//! The application owns this contract. Infrastructure implements it for
//! SQLite or another durable store. Implementations must preserve the
//! canonical [`Memory`](crate::domain::Memory) as authoritative state.

use std::fmt;

use crate::domain::{Memory, MemoryId};

/// Failure returned by a canonical-memory repository.
#[derive(Debug)]
pub enum MemoryRepositoryError {
    /// The backing store could not complete the requested operation.
    Storage(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for MemoryRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "memory repository storage error: {error}"),
        }
    }
}

impl std::error::Error for MemoryRepositoryError {}

/// Persistence port for canonical memories.
///
/// This port deliberately exposes domain values rather than database rows or
/// SQL concepts. Implementations must not make persistence depend on
/// embeddings, inference, or other derived intelligence.
pub trait MemoryRepository {
    /// Persists a newly constructed canonical memory.
    fn create(&self, memory: &Memory) -> Result<(), MemoryRepositoryError>;

    /// Loads one canonical memory by stable identity.
    fn get(&self, id: MemoryId) -> Result<Option<Memory>, MemoryRepositoryError>;

    /// Authoritatively deletes one canonical memory.
    ///
    /// Implementations are responsible for maintaining any persistence-level
    /// invariants required when canonical memory is removed.
    fn delete(&self, id: MemoryId) -> Result<(), MemoryRepositoryError>;
}
