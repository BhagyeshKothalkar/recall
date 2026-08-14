//! Port for durable background-job persistence and claiming.

use crate::domain::{Job, JobId, JobState, Timestamp};

/// Failure returned by a background-job repository.
#[derive(Debug)]
pub enum JobRepositoryError {
    /// The backing store could not complete the requested operation.
    Storage(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for JobRepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "job repository storage error: {error}"),
        }
    }
}

impl std::error::Error for JobRepositoryError {}

/// Persistence port for durable derivation jobs.
///
/// Job lifecycle semantics remain defined by the domain [`Job`] type.
/// Infrastructure is responsible for making claiming safe when multiple
/// workers or daemon activities can access the same queue.
pub trait JobRepository {
    /// Persists a newly created pending job.
    fn create(&self, job: &Job) -> Result<(), JobRepositoryError>;

    /// Atomically claims the next pending job, if one is available.
    ///
    /// A successful claim must return a job in `Running` state. The supplied
    /// timestamp is used for the domain transition.
    fn claim_pending(&self, now: Timestamp) -> Result<Option<Job>, JobRepositoryError>;

    /// Persists a job after a valid domain state transition.
    fn update(&self, job: &Job) -> Result<(), JobRepositoryError>;

    /// Loads one job by stable identity.
    fn get(&self, id: JobId) -> Result<Option<Job>, JobRepositoryError>;

    /// Returns jobs in the requested durable state.
    ///
    /// This is intended for status and operational views, not for bypassing
    /// the domain state machine.
    fn list_by_state(&self, state: JobState) -> Result<Vec<Job>, JobRepositoryError>;
}
