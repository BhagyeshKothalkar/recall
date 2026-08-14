//! SQLite persistence adapters.

mod connection;
mod memory_repository;
mod migrations;
mod search;

pub use connection::{DatabaseConnection, DatabaseError};
pub use memory_repository::SqliteMemoryRepository;
pub use search::SqliteMemorySearcher;
pub use migrations::{MigrationError, Migrator};
