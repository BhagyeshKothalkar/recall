//! Recall dependency composition root.
//!
//! Concrete infrastructure is assembled here. Application behavior remains
//! in application services; this module only decides which implementations
//! satisfy their ports.

use std::{path::Path, sync::Arc};

use crate::application::{
    ask::AskRecall,
    status::{ComponentStatus, GetStatus},
    store::{MemoryLimits, StoreMemory},
};
use crate::domain::InferenceModel;
use crate::infrastructure::clock::SystemClock;
use crate::infrastructure::database::{
    DatabaseError, SqliteEmbeddingRepository, SqliteHybridMemorySearcher, SqliteJobRepository,
    SqliteMemoryRepository, SqliteMemorySearcher, SqliteSemanticMemorySearcher,
    SqliteStorePersistence,
};
use crate::infrastructure::inference::{
    OllamaBackend, OllamaConfig, OllamaConfigError, OllamaError,
};
use crate::runtime::worker::EmbeddingWorker;

/// Concrete store service used by the daemon.
pub type StoreService = StoreMemory<SqliteStorePersistence, SystemClock>;

/// Concrete ask service used by the daemon.
pub type AskService = AskRecall<SqliteMemorySearcher, Arc<OllamaBackend>>;

/// Concrete status service used by the daemon.
pub type StatusService = GetStatus<SqliteJobRepository, SystemClock>;

/// Concrete embedding worker used by the daemon.
pub type WorkerService = EmbeddingWorker<
    SqliteJobRepository,
    SqliteMemoryRepository,
    SqliteEmbeddingRepository,
    OllamaBackend,
    SystemClock,
>;

/// Builds the canonical store service used by the daemon.
pub fn build_store_service(database_path: &Path) -> Result<StoreService, CompositionError> {
    let persistence =
        SqliteStorePersistence::open(database_path).map_err(CompositionError::Database)?;
    let clock = SystemClock;
    let limits = MemoryLimits::new(DEFAULT_MAX_MEMORY_BYTES);

    Ok(StoreMemory::new(persistence, clock, limits))
}

/// Builds the retrieval and inference service used by the daemon.
///
/// Ollama configuration is loaded exclusively by the inference configuration
/// module. The composition root only selects that concrete backend.
pub fn build_ask_service(database_path: &Path) -> Result<AskService, CompositionError> {
    let lexical = SqliteMemorySearcher::open(database_path).map_err(CompositionError::Database)?;
    let hybrid_lexical =
        SqliteMemorySearcher::open(database_path).map_err(CompositionError::Database)?;
    let config = OllamaConfig::from_env().map_err(CompositionError::OllamaConfig)?;
    let model = InferenceModel::new(config.embedding_model().to_owned()).ok_or(
        CompositionError::OllamaConfig(OllamaConfigError::EmptyEmbeddingModel),
    )?;
    let inference = Arc::new(OllamaBackend::new(config).map_err(CompositionError::Ollama)?);
    let semantic = SqliteSemanticMemorySearcher::open(database_path, Arc::clone(&inference), model)
        .map_err(CompositionError::Database)?;
    let hybrid = SqliteHybridMemorySearcher::new(hybrid_lexical, semantic);

    AskRecall::with_hybrid(
        lexical,
        Box::new(hybrid),
        inference,
        DEFAULT_RETRIEVAL_LIMIT,
    )
    .map_err(CompositionError::AskConfig)
}

/// Builds the daemon status service over the shared SQLite database.
pub fn build_status_service(database_path: &Path) -> Result<StatusService, CompositionError> {
    let jobs = SqliteJobRepository::open(database_path).map_err(CompositionError::Database)?;
    Ok(GetStatus::new(jobs, SystemClock, ComponentStatus::Ready))
}

/// Builds the durable embedding worker over the shared SQLite database.
pub fn build_worker(database_path: &Path) -> Result<WorkerService, CompositionError> {
    let config = OllamaConfig::from_env().map_err(CompositionError::OllamaConfig)?;
    let inference = OllamaBackend::new(config.clone()).map_err(CompositionError::Ollama)?;
    let model = InferenceModel::new(config.embedding_model().to_owned()).ok_or(
        CompositionError::OllamaConfig(OllamaConfigError::EmptyEmbeddingModel),
    )?;
    let jobs = SqliteJobRepository::open(database_path).map_err(CompositionError::Database)?;
    let memories =
        SqliteMemoryRepository::open(database_path).map_err(CompositionError::Database)?;
    let embeddings =
        SqliteEmbeddingRepository::open(database_path).map_err(CompositionError::Database)?;
    Ok(EmbeddingWorker::new(
        jobs,
        memories,
        embeddings,
        inference,
        SystemClock,
        model,
    ))
}

/// Initial conservative content limit for one canonical memory.
const DEFAULT_MAX_MEMORY_BYTES: usize = 16 * 1024 * 1024;

/// Initial number of lexical candidates supplied to generation or retrieval-only mode.
const DEFAULT_RETRIEVAL_LIMIT: usize = 8;

/// Errors produced while constructing daemon dependencies.
#[derive(Debug)]
pub enum CompositionError {
    /// Runtime filesystem setup failed.
    Filesystem(std::io::Error),
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
            Self::Filesystem(error) => write!(formatter, "runtime filesystem error: {error}"),
            Self::Database(error) => write!(formatter, "database composition error: {error}"),
            Self::OllamaConfig(error) => write!(formatter, "Ollama configuration error: {error}"),
            Self::Ollama(error) => write!(formatter, "Ollama composition error: {error}"),
            Self::AskConfig(error) => write!(formatter, "ask composition error: {error}"),
        }
    }
}

impl std::error::Error for CompositionError {}
