//! Execution of checked-in SQLite migrations.

use std::fmt;

use rusqlite::Connection;

const INITIAL_SCHEMA: &str = include_str!("../../../migrations/0001_memories.sql");
const FTS_SCHEMA: &str = include_str!("../../../migrations/0002_fts.sql");
const DERIVATION_SCHEMA: &str = include_str!("../../../migrations/0003_jobs_embeddings.sql");

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
        connection.execute_batch(DERIVATION_SCHEMA)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_canonical_and_derivation_schema() {
        let connection = Connection::open_in_memory().unwrap();
        Migrator::apply(&connection).unwrap();

        for table in ["memories", "jobs", "embeddings", "memories_fts"] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "missing table {table}");
        }
    }
}
