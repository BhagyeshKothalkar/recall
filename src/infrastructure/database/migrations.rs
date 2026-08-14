//! Execution of checked-in SQLite migrations.

use std::fmt;

use rusqlite::Connection;

const INITIAL_SCHEMA: &str = include_str!("../../../migrations/0001_memories.sql");
const FTS_SCHEMA: &str = include_str!("../../../migrations/0002_fts.sql");

/// Migration failures.
#[derive(Debug)]
pub enum MigrationError {
    /// SQLite rejected a migration operation.
    Sqlite(rusqlite::Error),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "migration error: {error}"),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<rusqlite::Error> for MigrationError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// Applies all migrations known to this application version.
pub struct Migrator;

impl Migrator {
    /// Applies the initial canonical-memory schema.
    pub fn apply(connection: &Connection) -> Result<(), MigrationError> {
        connection.execute_batch(INITIAL_SCHEMA)?;
        connection.execute_batch(FTS_SCHEMA)?;
        Ok(())
    }
}
