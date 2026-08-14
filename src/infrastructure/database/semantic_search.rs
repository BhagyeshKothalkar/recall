//! SQLite-backed semantic retrieval over durable embedding vectors.
//!
//! This deliberately uses SQLite for storage and computes cosine similarity
//! in the Recall process. A specialized vector extension can replace this
//! implementation later without changing the application port.

use std::{cmp::Ordering, fmt, path::Path};

use rusqlite::params;

use crate::{
    application::ports::{SemanticMemorySearcher, SearchError, TextEmbedder},
    domain::{InferenceModel, Memory, MemoryId, MemorySource, SearchQuery, SearchResult, SearchScore, Timestamp},
};

use super::{embedding_repository::decode_vector, DatabaseConnection, DatabaseError, Migrator};

/// SQLite semantic searcher using an injected text-embedding capability.
pub struct SqliteSemanticMemorySearcher<E> {
    database: DatabaseConnection,
    embedder: E,
    model: InferenceModel,
}

impl<E> SqliteSemanticMemorySearcher<E> {
    /// Opens the database and selects the model used by semantic retrieval.
    pub fn open(path: impl AsRef<Path>, embedder: E, model: InferenceModel) -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::open(path)?;
        Migrator::apply(database.connection()).map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database, embedder, model })
    }
}

impl<E: TextEmbedder> SemanticMemorySearcher for SqliteSemanticMemorySearcher<E> {
    fn search_semantic(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchError> {
        let query_vector = self.embedder.embed_text(query.text(), &self.model).map_err(|error| SearchError::Retrieval(Box::new(error)))?;
        let mut statement = self.database.connection().prepare(
            "SELECT m.id, m.content, m.source_type, m.source_value, m.created_at, m.updated_at, e.vector
             FROM embeddings e JOIN memories m ON m.id = e.memory_id
             WHERE e.model = ?1",
        ).map_err(storage_error)?;
        let rows = statement.query_map(params![self.model.name()], |row| {
            let vector: Vec<u8> = row.get(6)?;
            let vector = decode_vector(&vector).ok_or_else(|| rusqlite::Error::InvalidColumnType(6, "vector".into(), rusqlite::types::Type::Blob))?;
            let memory = read_memory(row)?;
            let similarity = cosine_similarity(&query_vector, &vector).ok_or_else(|| rusqlite::Error::InvalidColumnType(6, "vector".into(), rusqlite::types::Type::Blob))?;
            let score = SearchScore::new((similarity + 1.0) / 2.0).ok_or_else(|| rusqlite::Error::InvalidColumnType(6, "score".into(), rusqlite::types::Type::Real))?;
            Ok(SearchResult::new(memory, score))
        }).map_err(storage_error)?;
        let mut results = rows.collect::<Result<Vec<_>, _>>().map_err(storage_error)?;
        results.sort_by(|a, b| b.score().value().partial_cmp(&a.score().value()).unwrap_or(Ordering::Equal));
        results.truncate(query.limit());
        Ok(results)
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() { return None; }
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (&a, &b) in left.iter().zip(right) {
        if !a.is_finite() || !b.is_finite() { return None; }
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    let denominator = left_norm.sqrt() * right_norm.sqrt();
    if denominator == 0.0 { None } else { Some(dot / denominator) }
}

fn read_memory(row: &rusqlite::Row<'_>) -> rusqlite::Result<Memory> {
    let id = uuid::Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
    let source = match (row.get::<_, String>(2)?.as_str(), row.get::<_, Option<String>>(3)?) {
        ("direct_input", None) => MemorySource::DirectInput,
        ("file", Some(path)) => MemorySource::File { path: path.into() },
        _ => return Err(rusqlite::Error::InvalidColumnType(2, "source".into(), rusqlite::types::Type::Text)),
    };
    Memory::new(
        MemoryId::from_uuid(id), row.get(1)?, source,
        Timestamp::from_unix_millis(row.get(4)?), Timestamp::from_unix_millis(row.get(5)?),
    ).map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))
}

fn storage_error(error: rusqlite::Error) -> SearchError { SearchError::Retrieval(Box::new(error)) }

impl<E> fmt::Debug for SqliteSemanticMemorySearcher<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("SqliteSemanticMemorySearcher").field("model", &self.model.name()).finish_non_exhaustive() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::TextEmbedder;
    use crate::domain::MemorySource;

    struct FakeEmbedder;
    impl TextEmbedder for FakeEmbedder {
        fn embed_text(&self, text: &str, _model: &InferenceModel) -> Result<Vec<f32>, crate::application::ports::InferenceError> {
            Ok(if text.contains("rust") { vec![1.0, 0.0] } else { vec![0.0, 1.0] })
        }
    }

    #[test]
    fn semantic_search_prefers_similar_embedding() {
        let searcher = SqliteSemanticMemorySearcher::open(
            ":memory:", FakeEmbedder, InferenceModel::new("test".to_owned()).unwrap(),
        ).unwrap();
        let timestamp = Timestamp::from_unix_millis(1);
        let rust = Memory::new(MemoryId::new(), "Rust ownership".to_owned(), MemorySource::DirectInput, timestamp, timestamp).unwrap();
        let sqlite = Memory::new(MemoryId::new(), "SQLite WAL".to_owned(), MemorySource::DirectInput, timestamp, timestamp).unwrap();
        for (memory, vector) in [(&rust, vec![1.0, 0.0]), (&sqlite, vec![0.0, 1.0])] {
            searcher.database.connection().execute(
                "INSERT INTO memories (id, content, source_type, source_value, created_at, updated_at) VALUES (?1, ?2, 'direct_input', NULL, 1, 1)",
                params![memory.id().to_string(), memory.content()],
            ).unwrap();
            searcher.database.connection().execute(
                "INSERT INTO embeddings (memory_id, model, vector, dimensions, created_at) VALUES (?1, 'test', ?2, 2, 1)",
                params![memory.id().to_string(), super::super::embedding_repository::encode_vector(&vector)],
            ).unwrap();
        }
        let query = SearchQuery::new("rust language".to_owned(), 2).unwrap();
        let results = searcher.search_semantic(&query).unwrap();
        assert_eq!(results[0].memory_id(), rust.id());
    }
}
