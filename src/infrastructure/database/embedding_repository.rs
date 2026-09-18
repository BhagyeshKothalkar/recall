//! SQLite persistence for derived embedding vectors.

use std::{fmt, path::Path};

use rusqlite::params;

use crate::{
    application::ports::{EmbeddingRepository, EmbeddingRepositoryError},
    domain::{EmbeddingResponse, InferenceModel, MemoryId},
};

use super::{DatabaseConnection, DatabaseError, Migrator};

/// SQLite-backed embedding repository.
pub struct SqliteEmbeddingRepository {
    database: DatabaseConnection,
}

impl SqliteEmbeddingRepository {
    /// Opens and migrates a file-backed embedding store.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::open(path)?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }

    /// Creates an isolated embedding repository for tests.
    pub fn in_memory() -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::in_memory()?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }
}

impl EmbeddingRepository for SqliteEmbeddingRepository {
    fn upsert(&self, embedding: &EmbeddingResponse) -> Result<(), EmbeddingRepositoryError> {
        let vector = encode_vector(embedding.vector());
        self.database
            .connection()
            .execute(
                "INSERT INTO embeddings (memory_id, model, vector, dimensions, created_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%s','now') * 1000)
             ON CONFLICT(memory_id) DO UPDATE SET
                 model = excluded.model,
                 vector = excluded.vector,
                 dimensions = excluded.dimensions,
                 created_at = excluded.created_at",
                params![
                    embedding.memory_id().to_string(),
                    embedding.model().name(),
                    vector,
                    embedding.vector().len() as i64,
                ],
            )
            .map(|_| ())
            .map_err(storage_error)
    }

    fn delete(&self, memory_id: MemoryId) -> Result<(), EmbeddingRepositoryError> {
        self.database
            .connection()
            .execute(
                "DELETE FROM embeddings WHERE memory_id = ?1",
                params![memory_id.to_string()],
            )
            .map(|_| ())
            .map_err(storage_error)
    }

    fn exists_for_model(
        &self,
        memory_id: MemoryId,
        model: &InferenceModel,
    ) -> Result<bool, EmbeddingRepositoryError> {
        let count: i64 = self
            .database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM embeddings WHERE memory_id = ?1 AND model = ?2",
                params![memory_id.to_string(), model.name()],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        Ok(count != 0)
    }
}

/// Encodes f32 values without adding another serialization dependency.
pub(crate) fn encode_vector(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Decodes the little-endian f32 representation stored in SQLite.
pub(crate) fn decode_vector(bytes: &[u8]) -> Option<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    Some(
        bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect(),
    )
}

fn storage_error(error: rusqlite::Error) -> EmbeddingRepositoryError {
    EmbeddingRepositoryError::Storage(Box::new(error))
}

impl fmt::Debug for SqliteEmbeddingRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteEmbeddingRepository")
            .finish_non_exhaustive()
    }
}
