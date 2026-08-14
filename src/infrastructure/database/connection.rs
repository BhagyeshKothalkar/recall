//! SQLite connection setup and database-level configuration.

use std::{fmt, path::Path};

use rusqlite::{Connection, OpenFlags};

/// Database connection failures.
#[derive(Debug)]
pub enum DatabaseError {
    /// SQLite could not open or configure the database.
    Sqlite(rusqlite::Error),
    /// A checked-in migration could not be applied while constructing a repository.
    Migration(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "database error: {error}"),
            Self::Migration(error) => write!(formatter, "database migration error: {error}"),
        }
    }
}

impl std::error::Error for DatabaseError {}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// Owns one SQLite connection configured for Recall.
pub struct DatabaseConnection {
    connection: Connection,
}

impl DatabaseConnection {
    /// Opens a file-backed Recall database and enables WAL mode.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;

        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;

        Ok(Self { connection })
    }

    /// Opens an in-memory SQLite database for tests and local adapters.
    pub fn in_memory() -> Result<Self, DatabaseError> {
        let connection = Connection::open_in_memory()?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { connection })
    }

    /// Borrows the underlying SQLite connection for infrastructure setup.
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Consumes the wrapper and returns the SQLite connection.
    pub fn into_inner(self) -> Connection {
        self.connection
    }
}
