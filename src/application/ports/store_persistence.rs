//! Atomic persistence boundary for canonical storage plus its first derivation job.

use std::fmt;

use crate::domain::{Job, Memory};

/// Failure returned when the store transaction cannot be committed.
#[derive(Debug)]
pub enum StorePersistenceError {
    /// The backing store rejected the transaction.
    Storage(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for StorePersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "store persistence error: {error}"),
        }
    }
}

impl std::error::Error for StorePersistenceError {}

/// Atomic store boundary.
///
/// The canonical memory and its initial derivation job are committed together
/// so a successful store can never leave durable memory invisible to the job
/// system.
pub trait StorePersistence {
    /// Atomically persists canonical memory and its initial pending job.
    fn persist(&self, memory: &Memory, job: &Job) -> Result<(), StorePersistenceError>;
}
