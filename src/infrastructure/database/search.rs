//! SQLite FTS5 implementation of lexical memory retrieval.
//!
//! The FTS table is derived from canonical `memories` content. Search never
//! becomes the owner of memory state; it reconstructs `Memory` values from
//! the canonical table after selecting candidate rowids.

use std::{fmt, path::PathBuf};

use rusqlite::{params, Connection};

use crate::{
    application::ports::{MemorySearcher, SearchError},
    domain::{Memory, MemoryId, MemorySource, SearchQuery, SearchResult, SearchScore, Timestamp},
};

use super::{DatabaseConnection, DatabaseError, Migrator};

/// SQLite-backed lexical memory searcher using FTS5.
pub struct SqliteMemorySearcher {
    database: DatabaseConnection,
}

impl SqliteMemorySearcher {
    /// Opens and migrates a file-backed search database.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::open(path)?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }

    /// Creates an in-memory lexical search database for tests.
    pub fn in_memory() -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::in_memory()?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }

    /// Borrows the SQLite connection for infrastructure-level setup/tests.
    pub(crate) fn connection(&self) -> &Connection {
        self.database.connection()
    }
}

impl MemorySearcher for SqliteMemorySearcher {
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchError> {
        let match_expression = fts_match_expression(query.text());
        if match_expression.is_empty() {
            return Ok(Vec::new());
        }

        let mut statement = self
            .connection()
            .prepare(
                "SELECT m.id, m.content, m.source_type, m.source_value,
                        m.created_at, m.updated_at, bm25(memories_fts) AS rank
                 FROM memories_fts
                 JOIN memories AS m ON m.rowid = memories_fts.rowid
                 WHERE memories_fts MATCH ?1
                 ORDER BY rank ASC
                 LIMIT ?2",
            )
            .map_err(storage_error)?;

        let rows = statement
            .query_map(params![match_expression, query.limit() as i64], |row| {
                let memory = read_memory(row)?;
                let rank = row.get::<_, f64>(6)? as f32;
                let score = SearchScore::new(-rank).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Real,
                        "FTS returned an invalid score".into(),
                    )
                })?;
                Ok(SearchResult::new(memory, score))
            })
            .map_err(storage_error)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(storage_error)
    }
}

fn fts_match_expression(text: &str) -> String {
    text.split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
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

fn storage_error(error: rusqlite::Error) -> SearchError {
    SearchError::Retrieval(Box::new(error))
}

impl fmt::Debug for SqliteMemorySearcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteMemorySearcher")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lexical_search_returns_matching_memory() {
        let searcher = SqliteMemorySearcher::in_memory().unwrap();
        let timestamp = Timestamp::from_unix_millis(100);
        let memory = Memory::new(
            MemoryId::new(),
            "SQLite WAL mode improves durability.".to_owned(),
            MemorySource::DirectInput,
            timestamp,
            timestamp,
        )
        .unwrap();

        searcher.connection().execute(
            "INSERT INTO memories (id, content, source_type, source_value, created_at, updated_at)
             VALUES (?1, ?2, 'direct_input', NULL, ?3, ?3)",
            params![memory.id().to_string(), memory.content(), timestamp.as_unix_millis()],
        ).unwrap();

        let query = SearchQuery::new("SQLite".to_owned(), 5).unwrap();
        let results = searcher.search(&query).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory_id(), memory.id());
        assert!(results[0].score().value().is_finite());
    }

    #[test]
    fn lexical_search_observes_canonical_deletion() {
        let searcher = SqliteMemorySearcher::in_memory().unwrap();
        let timestamp = Timestamp::from_unix_millis(100);
        let memory = Memory::new(
            MemoryId::new(),
            "temporary searchable memory".to_owned(),
            MemorySource::DirectInput,
            timestamp,
            timestamp,
        )
        .unwrap();

        searcher.connection().execute(
            "INSERT INTO memories (id, content, source_type, source_value, created_at, updated_at)
             VALUES (?1, ?2, 'direct_input', NULL, ?3, ?3)",
            params![memory.id().to_string(), memory.content(), timestamp.as_unix_millis()],
        ).unwrap();
        searcher
            .connection()
            .execute(
                "DELETE FROM memories WHERE id = ?1",
                params![memory.id().to_string()],
            )
            .unwrap();

        let query = SearchQuery::new("temporary".to_owned(), 5).unwrap();
        assert!(searcher.search(&query).unwrap().is_empty());
    }
}
