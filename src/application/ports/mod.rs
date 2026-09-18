//! Application-owned ports for infrastructure capabilities.
//!
//! The application defines what it needs; infrastructure supplies concrete
//! implementations. These ports therefore depend only on domain types and
//! standard library abstractions, never on SQLite, Ollama, IPC, or CLI code.

mod clock;
mod embedding_repository;
mod inference_backend;
mod job_repository;
mod memory_repository;
mod memory_searcher;
mod semantic_memory_searcher;
mod store_persistence;
mod text_embedder;

pub use clock::Clock;
pub use embedding_repository::{EmbeddingRepository, EmbeddingRepositoryError};
pub use inference_backend::{InferenceBackend, InferenceError};
pub use job_repository::{JobRepository, JobRepositoryError};
pub use memory_repository::{MemoryRepository, MemoryRepositoryError};
pub use memory_searcher::{MemorySearcher, SearchError};
pub use semantic_memory_searcher::SemanticMemorySearcher;
pub use store_persistence::{StorePersistence, StorePersistenceError};
pub use text_embedder::TextEmbedder;
