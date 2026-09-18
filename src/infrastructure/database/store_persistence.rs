//! SQLite atomic persistence for canonical memory plus initial derivation job.

use std::{fmt, path::Path};

use rusqlite::params;

use crate::{
    application::ports::{StorePersistence, StorePersistenceError},
    domain::{Job, JobKind, Memory},
};

use super::{DatabaseConnection, DatabaseError, Migrator};

/// SQLite implementation of the atomic store persistence boundary.
pub struct SqliteStorePersistence {
    database: DatabaseConnection,
}

impl SqliteStorePersistence {
    /// Opens and migrates the database used by the store transaction.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::open(path)?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }

    /// Creates an isolated store persistence database for tests.
    pub fn in_memory() -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::in_memory()?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }
}

impl StorePersistence for SqliteStorePersistence {
    fn persist(&self, memory: &Memory, job: &Job) -> Result<(), StorePersistenceError> {
        let connection = self.database.connection();
        let tx = connection.unchecked_transaction().map_err(storage_error)?;

        tx.execute(
            "INSERT INTO memories (id, content, source_type, source_value, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                memory.id().to_string(),
                memory.content(),
                source_type(memory),
                source_value(memory),
                memory.created_at().as_unix_millis(),
                memory.updated_at().as_unix_millis(),
            ],
        )
        .map_err(storage_error)?;

        let JobKind::GenerateEmbedding { memory_id } = job.kind();
        tx.execute(
            "INSERT INTO jobs (id, kind, memory_id, state, attempts, created_at, updated_at, last_error)
             VALUES (?1, 'generate_embedding', ?2, 'pending', 0, ?3, ?3, NULL)",
            params![job.id().to_string(), memory_id.to_string(), job.created_at().as_unix_millis()],
        ).map_err(storage_error)?;

        tx.commit().map_err(storage_error)
    }
}

fn source_type(memory: &Memory) -> &'static str {
    match memory.source() {
        crate::domain::MemorySource::DirectInput => "direct_input",
        crate::domain::MemorySource::File { .. } => "file",
    }
}

fn source_value(memory: &Memory) -> Option<String> {
    match memory.source() {
        crate::domain::MemorySource::DirectInput => None,
        crate::domain::MemorySource::File { path } => Some(path.to_string_lossy().into_owned()),
    }
}

fn storage_error(error: rusqlite::Error) -> StorePersistenceError {
    StorePersistenceError::Storage(Box::new(error))
}

impl fmt::Debug for SqliteStorePersistence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteStorePersistence")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{JobId, MemoryId, MemorySource, Timestamp};

    #[test]
    fn persists_memory_and_job_in_one_database() {
        let persistence = SqliteStorePersistence::in_memory().unwrap();
        let timestamp = Timestamp::from_unix_millis(10);
        let memory = Memory::new(
            MemoryId::new(),
            "hello".to_owned(),
            MemorySource::DirectInput,
            timestamp,
            timestamp,
        )
        .unwrap();
        let job = Job::new(
            JobId::new(),
            JobKind::GenerateEmbedding {
                memory_id: memory.id(),
            },
            timestamp,
        );

        persistence.persist(&memory, &job).unwrap();

        let connection = persistence.database.connection();
        let memory_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        let job_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(memory_count, 1);
        assert_eq!(job_count, 1);
    }

    #[test]
    fn duplicate_memory_rolls_back_job_insert() {
        let persistence = SqliteStorePersistence::in_memory().unwrap();
        let timestamp = Timestamp::from_unix_millis(10);
        let memory = Memory::new(
            MemoryId::new(),
            "hello".to_owned(),
            MemorySource::DirectInput,
            timestamp,
            timestamp,
        )
        .unwrap();
        let first_job = Job::new(
            JobId::new(),
            JobKind::GenerateEmbedding {
                memory_id: memory.id(),
            },
            timestamp,
        );
        let second_job = Job::new(
            JobId::new(),
            JobKind::GenerateEmbedding {
                memory_id: memory.id(),
            },
            timestamp,
        );

        persistence.persist(&memory, &first_job).unwrap();
        assert!(persistence.persist(&memory, &second_job).is_err());

        let job_count: i64 = persistence
            .database
            .connection()
            .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(job_count, 1);
    }
}
