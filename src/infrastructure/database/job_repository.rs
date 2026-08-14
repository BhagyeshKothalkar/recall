//! SQLite persistence for durable derivation jobs.

use std::{fmt, path::Path};

use rusqlite::{params, OptionalExtension};

use crate::{
    application::ports::{JobRepository, JobRepositoryError},
    domain::{Job, JobId, JobKind, JobState, MemoryId, Timestamp},
};

use super::{DatabaseConnection, DatabaseError, Migrator};

/// SQLite-backed durable job repository.
pub struct SqliteJobRepository {
    database: DatabaseConnection,
}

impl SqliteJobRepository {
    /// Opens and migrates a file-backed job database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::open(path)?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }

    /// Creates an isolated job repository for tests.
    pub fn in_memory() -> Result<Self, DatabaseError> {
        let database = DatabaseConnection::in_memory()?;
        Migrator::apply(database.connection())
            .map_err(|error| DatabaseError::Migration(Box::new(error)))?;
        Ok(Self { database })
    }
}

impl JobRepository for SqliteJobRepository {
    fn create(&self, job: &Job) -> Result<(), JobRepositoryError> {
        self.database.connection().execute(
            "INSERT INTO jobs (id, kind, memory_id, state, attempts, created_at, updated_at, last_error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                job.id().to_string(),
                kind_name(job.kind()),
                memory_id(job.kind()).to_string(),
                state_name(job.state()),
                job.attempts() as i64,
                job.created_at().as_unix_millis(),
                job.updated_at().as_unix_millis(),
                job.last_error(),
            ],
        ).map(|_| ()).map_err(storage_error)
    }

    fn claim_pending(&self, now: Timestamp) -> Result<Option<Job>, JobRepositoryError> {
        let connection = self.database.connection();
        let tx = connection.unchecked_transaction().map_err(storage_error)?;
        let id = tx.query_row(
            "SELECT id FROM jobs WHERE state = 'pending' ORDER BY created_at ASC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        ).optional().map_err(storage_error)?;

        let Some(id) = id else {
            tx.commit().map_err(storage_error)?;
            return Ok(None);
        };

        tx.execute(
            "UPDATE jobs SET state = 'running', attempts = attempts + 1, updated_at = ?2
             WHERE id = ?1 AND state = 'pending'",
            params![id, now.as_unix_millis()],
        ).map_err(storage_error)?;

        let job = tx.query_row(
            "SELECT id, kind, memory_id, state, attempts, created_at, updated_at, last_error
             FROM jobs WHERE id = ?1",
            params![id],
            read_job,
        ).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;
        Ok(Some(job))
    }

    fn update(&self, job: &Job) -> Result<(), JobRepositoryError> {
        let changed = self.database.connection().execute(
            "UPDATE jobs SET state = ?2, attempts = ?3, updated_at = ?4, last_error = ?5 WHERE id = ?1",
            params![
                job.id().to_string(),
                state_name(job.state()),
                job.attempts() as i64,
                job.updated_at().as_unix_millis(),
                job.last_error(),
            ],
        ).map_err(storage_error)?;
        if changed == 0 {
            return Err(JobRepositoryError::Storage(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "job does not exist"))));
        }
        Ok(())
    }

    fn get(&self, id: JobId) -> Result<Option<Job>, JobRepositoryError> {
        self.database.connection().query_row(
            "SELECT id, kind, memory_id, state, attempts, created_at, updated_at, last_error
             FROM jobs WHERE id = ?1",
            params![id.to_string()],
            read_job,
        ).optional().map_err(storage_error)
    }

    fn list_by_state(&self, state: JobState) -> Result<Vec<Job>, JobRepositoryError> {
        let mut statement = self.database.connection().prepare(
            "SELECT id, kind, memory_id, state, attempts, created_at, updated_at, last_error
             FROM jobs WHERE state = ?1 ORDER BY created_at ASC",
        ).map_err(storage_error)?;
        let rows = statement.query_map(params![state_name(state)], read_job).map_err(storage_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage_error)
    }
}

fn read_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    let id = JobId::from_uuid(uuid::Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error)))?);
    let kind_name = row.get::<_, String>(1)?;
    let memory_id = MemoryId::from_uuid(uuid::Uuid::parse_str(&row.get::<_, String>(2)?).map_err(|error| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(error)))?);
    let kind = match kind_name.as_str() { "generate_embedding" => JobKind::GenerateEmbedding { memory_id }, _ => return Err(rusqlite::Error::InvalidColumnType(1, "kind".into(), rusqlite::types::Type::Text)) };
    let state = match row.get::<_, String>(3)?.as_str() { "pending" => JobState::Pending, "running" => JobState::Running, "completed" => JobState::Completed, "failed" => JobState::Failed, _ => return Err(rusqlite::Error::InvalidColumnType(3, "state".into(), rusqlite::types::Type::Text)) };
    Job::from_persisted(id, kind, state, Timestamp::from_unix_millis(row.get(5)?), Timestamp::from_unix_millis(row.get(6)?), row.get::<_, i64>(4)? as u32, row.get(7)?).map_err(|error| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error)))
}

fn kind_name(kind: JobKind) -> &'static str { match kind { JobKind::GenerateEmbedding { .. } => "generate_embedding" } }
fn memory_id(kind: JobKind) -> MemoryId { match kind { JobKind::GenerateEmbedding { memory_id } => memory_id } }
fn state_name(state: JobState) -> &'static str { match state { JobState::Pending => "pending", JobState::Running => "running", JobState::Completed => "completed", JobState::Failed => "failed" } }
fn storage_error(error: rusqlite::Error) -> JobRepositoryError { JobRepositoryError::Storage(Box::new(error)) }

impl fmt::Debug for SqliteJobRepository { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.debug_struct("SqliteJobRepository").finish_non_exhaustive() } }
