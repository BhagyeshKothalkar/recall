//! SQLite persistence adapters.

mod connection;
mod embedding_repository;
mod hybrid_search;
mod job_repository;
mod memory_repository;
mod migrations;
mod search;
mod semantic_search;
mod store_persistence;

pub use connection::{DatabaseConnection, DatabaseError};
pub use embedding_repository::SqliteEmbeddingRepository;
pub use hybrid_search::{HybridWithOllama, SqliteHybridMemorySearcher};
pub use job_repository::SqliteJobRepository;
pub use memory_repository::SqliteMemoryRepository;
pub use migrations::{MigrationError, Migrator};
pub use search::SqliteMemorySearcher;
pub use semantic_search::SqliteSemanticMemorySearcher;
pub use store_persistence::SqliteStorePersistence;
