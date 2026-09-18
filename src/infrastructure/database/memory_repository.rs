//! SQLite implementation of canonical memory persistence.

use std::{fmt, path::PathBuf};

use rusqlite::{params, Connection};

use crate::{
    application::ports::{MemoryRepository, MemoryRepositoryError},
    domain::{Memory, MemoryId, MemorySource, Timestamp},
};

use super::{DatabaseConnection, DatabaseError, Migrator};

/// SQLite-backed implementation of the canonical memory repository.
///
/// The repository owns its configured database connection so the daemon can
/// own the repository as one long-lived application dependency. This avoids a
/// self-referential daemon structure in which the repository borrows a field
/// owned by the same daemon.
pub struct SqliteMemoryRepository {
    database: DatabaseConnection,
}

impl SqliteMemoryRepository {
    /// Opens and migrates a file-backed canonical-memory database.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::open(path)?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }

    /// Creates a repository over an in-memory database for tests.
    pub fn in_memory() -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::in_memory()?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }

    /// Borrows the SQLite connection for infrastructure-level operations.
    pub(crate) fn connection(&self) -> &Connection {
        self.database.connection()
    }
}

impl MemoryRepository for SqliteMemoryRepository {
    fn create(&self, memory: &Memory) -> Result<(), MemoryRepositoryError> {
        self.connection()
            .execute(
                "INSERT INTO memories (
                    id, content, source_type, source_value, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    memory.id().to_string(),
                    memory.content(),
                    source_type(memory.source()),
                    source_value(memory.source()),
                    memory.created_at().as_unix_millis(),
                    memory.updated_at().as_unix_millis(),
                ],
            )
            .map(|_| ())
            .map_err(storage_error)
    }

    fn get(&self, id: MemoryId) -> Result<Option<Memory>, MemoryRepositoryError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT id, content, source_type, source_value, created_at, updated_at
                 FROM memories WHERE id = ?1",
            )
            .map_err(storage_error)?;

        let mut rows = statement
            .query(params![id.to_string()])
            .map_err(storage_error)?;

        match rows.next().map_err(storage_error)? {
            Some(row) => read_memory(row).map(Some).map_err(storage_error),
            None => Ok(None),
        }
    }

    fn delete(&self, id: MemoryId) -> Result<(), MemoryRepositoryError> {
        self.connection()
            .execute(
                "DELETE FROM memories WHERE id = ?1",
                params![id.to_string()],
            )
            .map(|_| ())
            .map_err(storage_error)
    }
}

fn source_type(source: &MemorySource) -> &'static str {
    match source {
        MemorySource::DirectInput => "direct_input",
        MemorySource::File { .. } => "file",
    }
}

fn source_value(source: &MemorySource) -> Option<String> {
    match source {
        MemorySource::DirectInput => None,
        MemorySource::File { path } => Some(path.to_string_lossy().into_owned()),
    }
}

fn read_memory(row: &rusqlite::Row<'_>) -> rusqlite::Result<Memory> {
    let id = row.get::<_, String>(0)?;
    let id = uuid::Uuid::parse_str(&id).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;

    let content = row.get::<_, String>(1)?;
    let source_type = row.get::<_, String>(2)?;
    let source_value = row.get::<_, Option<String>>(3)?;
    let created_at = Timestamp::from_unix_millis(row.get(4)?);
    let updated_at = Timestamp::from_unix_millis(row.get(5)?);

    let source = match (source_type.as_str(), source_value) {
        ("direct_input", None) => MemorySource::DirectInput,
        ("file", Some(path)) => MemorySource::File {
            path: PathBuf::from(path),
        },
        _ => {
            return Err(rusqlite::Error::InvalidColumnType(
                2,
                "source_type/source_value".to_owned(),
                rusqlite::types::Type::Text,
            ))
        }
    };

    Memory::new(
        MemoryId::from_uuid(id),
        content,
        source,
        created_at,
        updated_at,
    )
    .map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn storage_error(error: rusqlite::Error) -> MemoryRepositoryError {
    MemoryRepositoryError::Storage(Box::new(error))
}

impl fmt::Debug for SqliteMemoryRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteMemoryRepository")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_direct_memory() {
        let repository = SqliteMemoryRepository::in_memory().unwrap();
        let timestamp = Timestamp::from_unix_millis(100);
        let memory = Memory::new(
            MemoryId::new(),
            "hello".to_owned(),
            MemorySource::DirectInput,
            timestamp,
            timestamp,
        )
        .unwrap();

        repository.create(&memory).unwrap();

        assert_eq!(repository.get(memory.id()).unwrap(), Some(memory));
    }

    #[test]
    fn round_trips_file_source() {
        let repository = SqliteMemoryRepository::in_memory().unwrap();
        let timestamp = Timestamp::from_unix_millis(100);
        let memory = Memory::new(
            MemoryId::new(),
            "hello".to_owned(),
            MemorySource::File {
                path: PathBuf::from("notes/idea.txt"),
            },
            timestamp,
            timestamp,
        )
        .unwrap();

        repository.create(&memory).unwrap();

        assert_eq!(repository.get(memory.id()).unwrap(), Some(memory));
    }

    #[test]
    fn delete_removes_canonical_memory() {
        let repository = SqliteMemoryRepository::in_memory().unwrap();
        let timestamp = Timestamp::from_unix_millis(100);
        let memory = Memory::new(
            MemoryId::new(),
            "hello".to_owned(),
            MemorySource::DirectInput,
            timestamp,
            timestamp,
        )
        .unwrap();

        repository.create(&memory).unwrap();
        repository.delete(memory.id()).unwrap();

        assert_eq!(repository.get(memory.id()).unwrap(), None);
    }
}
