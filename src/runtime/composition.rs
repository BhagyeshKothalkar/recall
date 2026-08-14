//! Recall dependency composition root.
//!
//! Concrete infrastructure is assembled here. Application behavior remains
//! in application services; this module only decides which implementations
//! satisfy their ports.

use std::path::Path;

use crate::application::{ask::AskRecall, store::{MemoryLimits, StoreMemory}};
use crate::infrastructure::clock::SystemClock;
use crate::infrastructure::database::{DatabaseError, SqliteMemoryRepository, SqliteMemorySearcher};
use crate::infrastructure::inference::{OllamaBackend, OllamaConfig, OllamaConfigError, OllamaError};

/// Concrete store service used by the daemon.
pub type StoreService = StoreMemory<SqliteMemoryRepository, SystemClock>;

/// Concrete ask service used by the daemon.
pub type AskService = AskRecall<SqliteMemorySearcher, OllamaBackend>;

/// Builds the canonical store service used by the daemon.
pub fn build_store_service(database_path: &Path) -> Result<StoreService, CompositionError> {
    let repository = SqliteMemoryRepository::open(database_path)
        .map_err(CompositionError::Database)?;
    let clock = SystemClock;
    let limits = MemoryLimits::new(DEFAULT_MAX_MEMORY_BYTES);

    Ok(StoreMemory::new(repository, clock, limits))
}

/// Builds the retrieval and inference service used by the daemon.
///
/// Ollama configuration is loaded exclusively by the inference configuration
/// module. The composition root only selects that concrete backend.
pub fn build_ask_service(database_path: &Path) -> Result<AskService, CompositionError> {
    let searcher = SqliteMemorySearcher::open(database_path)
        .map_err(CompositionError::Database)?;
    let config = OllamaConfig::from_env().map_err(CompositionError::OllamaConfig)?;
    let inference = OllamaBackend::new(config).map_err(CompositionError::Ollama)?;

    AskRecall::new(searcher, inference, DEFAULT_RETRIEVAL_LIMIT)
        .map_err(CompositionError::AskConfig)
}

/// Initial conservative content limit for one canonical memory.
const DEFAULT_MAX_MEMORY_BYTES: usize = 16 * 1024 * 1024;

/// Initial number of lexical candidates supplied to generation or retrieval-only mode.
const DEFAULT_RETRIEVAL_LIMIT: usize = 8;

/// Errors produced while constructing daemon dependencies.
#[derive(Debug)]
pub enum CompositionError {
    /// SQLite setup or migration failed.
    Database(DatabaseError),
    /// Ollama configuration is invalid.
    OllamaConfig(OllamaConfigError),
    /// The Ollama HTTP adapter could not be constructed.
    Ollama(OllamaError),
    /// Ask-service configuration is invalid.
    AskConfig(crate::application::ask::AskConfigError),
}

impl std::fmt::Display for CompositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database composition error: {error}"),
            Self::OllamaConfig(error) => write!(formatter, "Ollama configuration error: {error}"),
            Self::Ollama(error) => write!(formatter, "Ollama composition error: {error}"),
            Self::AskConfig(error) => write!(formatter, "ask composition error: {error}"),
        }
    }
}

impl std::error::Error for CompositionError {}
