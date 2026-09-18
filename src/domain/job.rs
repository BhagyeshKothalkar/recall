//! Durable background-job domain types and state transitions.

use std::fmt;

use uuid::Uuid;

use super::memory::{MemoryId, Timestamp};

/// Stable identity of a persisted background job.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct JobId(Uuid);

impl JobId {
    /// Creates a new randomly generated job identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing UUID as a job identifier.
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Kind of derivation work currently supported by Recall.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobKind {
    /// Generate an embedding for one canonical memory.
    GenerateEmbedding { memory_id: MemoryId },
}

/// Durable lifecycle state of a background job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobState {
    /// Job has been persisted but has not started execution.
    Pending,
    /// A worker currently owns execution of the job.
    Running,
    /// Work completed successfully.
    Completed,
    /// Work failed; the recorded error may be used by retry policy.
    Failed,
}

/// Invalid state transition for a persisted job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidJobTransition {
    pub from: JobState,
    pub to: JobState,
}

impl fmt::Display for InvalidJobTransition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid job transition: {:?} -> {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for InvalidJobTransition {}

/// Invalid persisted job state reconstructed from storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobValidationError {
    /// A job's update timestamp predates its creation timestamp.
    InvalidTimestampOrder {
        created_at: Timestamp,
        updated_at: Timestamp,
    },
    /// Failed jobs must retain a diagnostic.
    FailedJobMissingError,
    /// Non-failed jobs must not retain a stale diagnostic.
    UnexpectedError,
}

impl fmt::Display for JobValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimestampOrder {
                created_at,
                updated_at,
            } => write!(
                formatter,
                "job updated_at ({updated_at}) cannot precede created_at ({created_at})"
            ),
            Self::FailedJobMissingError => formatter.write_str("failed job has no error"),
            Self::UnexpectedError => formatter.write_str("non-failed job has an error"),
        }
    }
}

impl std::error::Error for JobValidationError {}

/// Durable description of one unit of background derivation work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Job {
    id: JobId,
    kind: JobKind,
    state: JobState,
    created_at: Timestamp,
    updated_at: Timestamp,
    attempts: u32,
    last_error: Option<String>,
}

impl Job {
    /// Creates a new pending job.
    pub fn new(id: JobId, kind: JobKind, created_at: Timestamp) -> Self {
        Self {
            id,
            kind,
            state: JobState::Pending,
            created_at,
            updated_at: created_at,
            attempts: 0,
            last_error: None,
        }
    }

    /// Reconstructs a job from persisted state after validating invariants.
    pub fn from_persisted(
        id: JobId,
        kind: JobKind,
        state: JobState,
        created_at: Timestamp,
        updated_at: Timestamp,
        attempts: u32,
        last_error: Option<String>,
    ) -> Result<Self, JobValidationError> {
        if updated_at < created_at {
            return Err(JobValidationError::InvalidTimestampOrder {
                created_at,
                updated_at,
            });
        }
        if state == JobState::Failed && last_error.is_none() {
            return Err(JobValidationError::FailedJobMissingError);
        }
        if state != JobState::Failed && last_error.is_some() {
            return Err(JobValidationError::UnexpectedError);
        }
        Ok(Self {
            id,
            kind,
            state,
            created_at,
            updated_at,
            attempts,
            last_error,
        })
    }

    /// Returns the stable job identifier.
    pub const fn id(&self) -> JobId {
        self.id
    }

    /// Returns the work represented by the job.
    pub const fn kind(&self) -> JobKind {
        self.kind
    }

    /// Returns the current durable state.
    pub const fn state(&self) -> JobState {
        self.state
    }

    /// Returns when the job was created.
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns when the job was last changed.
    pub const fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    /// Returns the number of execution attempts.
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Returns the most recent failure, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Claims a pending job for execution.
    pub fn start(&mut self, now: Timestamp) -> Result<(), InvalidJobTransition> {
        self.transition_to(JobState::Running, now)?;
        self.attempts = self.attempts.saturating_add(1);
        Ok(())
    }

    /// Marks a running job as completed and clears any previous failure.
    pub fn complete(&mut self, now: Timestamp) -> Result<(), InvalidJobTransition> {
        self.transition_to(JobState::Completed, now)?;
        self.last_error = None;
        Ok(())
    }

    /// Marks a running job as failed and records the backend/application error.
    pub fn fail(&mut self, now: Timestamp, error: String) -> Result<(), InvalidJobTransition> {
        self.transition_to(JobState::Failed, now)?;
        self.last_error = Some(error);
        Ok(())
    }

    /// Requeues a failed job for another execution attempt.
    pub fn retry(&mut self, now: Timestamp) -> Result<(), InvalidJobTransition> {
        self.transition_to(JobState::Pending, now)
    }

    fn transition_to(
        &mut self,
        next: JobState,
        now: Timestamp,
    ) -> Result<(), InvalidJobTransition> {
        if !is_valid_transition(self.state, next) {
            return Err(InvalidJobTransition {
                from: self.state,
                to: next,
            });
        }

        self.state = next;
        self.updated_at = now;
        Ok(())
    }
}

fn is_valid_transition(from: JobState, to: JobState) -> bool {
    matches!(
        (from, to),
        (JobState::Pending, JobState::Running)
            | (JobState::Running, JobState::Completed)
            | (JobState::Running, JobState::Failed)
            | (JobState::Failed, JobState::Pending)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> Job {
        Job::new(
            JobId::new(),
            JobKind::GenerateEmbedding {
                memory_id: MemoryId::new(),
            },
            Timestamp::from_unix_millis(10),
        )
    }

    #[test]
    fn job_follows_expected_lifecycle() {
        let mut job = job();

        assert_eq!(job.state(), JobState::Pending);
        job.start(Timestamp::from_unix_millis(20)).unwrap();
        assert_eq!(job.state(), JobState::Running);
        assert_eq!(job.attempts(), 1);

        job.complete(Timestamp::from_unix_millis(30)).unwrap();
        assert_eq!(job.state(), JobState::Completed);
    }

    #[test]
    fn completed_job_cannot_run_again() {
        let mut job = job();
        job.start(Timestamp::from_unix_millis(20)).unwrap();
        job.complete(Timestamp::from_unix_millis(30)).unwrap();

        let result = job.start(Timestamp::from_unix_millis(40));

        assert_eq!(
            result,
            Err(InvalidJobTransition {
                from: JobState::Completed,
                to: JobState::Running,
            })
        );
    }

    #[test]
    fn failed_job_can_be_requeued() {
        let mut job = job();
        job.start(Timestamp::from_unix_millis(20)).unwrap();
        job.fail(
            Timestamp::from_unix_millis(30),
            "model unavailable".to_owned(),
        )
        .unwrap();
        assert_eq!(job.last_error(), Some("model unavailable"));

        job.retry(Timestamp::from_unix_millis(40)).unwrap();
        assert_eq!(job.state(), JobState::Pending);
    }
}
