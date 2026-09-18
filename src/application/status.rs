//! Application-owned daemon status types.
//!
//! These values describe operational state without exposing database rows,
//! IPC wire structs, or inference implementation details. Runtime adapters
//! can populate them; the application remains responsible for their meaning.

use std::fmt;

use super::ports::{Clock, JobRepository, JobRepositoryError};
use crate::domain::{JobState, Timestamp};

/// Health of one daemon-owned subsystem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComponentStatus {
    /// The subsystem is available for its supported operations.
    Ready,
    /// The subsystem is unavailable, with a safe diagnostic for operators.
    Unavailable(String),
}

impl ComponentStatus {
    /// Returns whether this component is ready.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Constructs an unavailable status from a displayable diagnostic.
    pub fn unavailable(error: impl fmt::Display) -> Self {
        Self::Unavailable(error.to_string())
    }

    /// Returns the diagnostic when the component is unavailable.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ready => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

/// Counts of durable derivation jobs grouped by lifecycle state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct JobCounts {
    pending: u64,
    running: u64,
    completed: u64,
    failed: u64,
}

impl JobCounts {
    /// Creates a complete job-count snapshot.
    pub const fn new(pending: u64, running: u64, completed: u64, failed: u64) -> Self {
        Self {
            pending,
            running,
            completed,
            failed,
        }
    }

    pub const fn pending(self) -> u64 {
        self.pending
    }
    pub const fn running(self) -> u64 {
        self.running
    }
    pub const fn completed(self) -> u64 {
        self.completed
    }
    pub const fn failed(self) -> u64 {
        self.failed
    }

    /// Returns the number of jobs represented by this snapshot.
    pub const fn total(self) -> u64 {
        self.pending + self.running + self.completed + self.failed
    }
}

/// Application-level operational status produced by the daemon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationStatus {
    checked_at: Timestamp,
    ready: bool,
    database: ComponentStatus,
    jobs: JobCounts,
    inference: ComponentStatus,
}

impl ApplicationStatus {
    /// Creates a status snapshot from independently probed components.
    ///
    /// Inference is reported separately because lexical retrieval and
    /// canonical storage can remain usable when an AI backend is unavailable.
    /// Overall readiness therefore depends on the database only.
    pub fn new(
        checked_at: Timestamp,
        database: ComponentStatus,
        jobs: JobCounts,
        inference: ComponentStatus,
    ) -> Self {
        let ready = database.is_ready();
        Self {
            checked_at,
            ready,
            database,
            jobs,
            inference,
        }
    }

    pub const fn checked_at(&self) -> Timestamp {
        self.checked_at
    }
    pub const fn ready(&self) -> bool {
        self.ready
    }
    pub const fn database(&self) -> &ComponentStatus {
        &self.database
    }
    pub const fn jobs(&self) -> JobCounts {
        self.jobs
    }
    pub const fn inference(&self) -> &ComponentStatus {
        &self.inference
    }
}

/// Failure while collecting application status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatusError {
    /// A named status component could not be probed.
    Probe {
        component: StatusComponent,
        message: String,
    },
}

impl StatusError {
    pub fn probe(component: StatusComponent, error: impl fmt::Display) -> Self {
        Self::Probe {
            component,
            message: error.to_string(),
        }
    }
}

impl fmt::Display for StatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe { component, message } => {
                write!(
                    formatter,
                    "{} status probe failed: {message}",
                    component.as_str()
                )
            }
        }
    }
}

impl std::error::Error for StatusError {}

/// Component names used in status diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusComponent {
    Database,
    Jobs,
    Inference,
}

impl StatusComponent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Jobs => "jobs",
            Self::Inference => "inference",
        }
    }
}

/// Application use case for collecting daemon operational status.
pub struct GetStatus<J, C> {
    jobs: J,
    clock: C,
    inference: ComponentStatus,
}

impl<J, C> GetStatus<J, C>
where
    J: JobRepository,
    C: Clock,
{
    /// Creates a status service with a static inference readiness snapshot.
    pub fn new(jobs: J, clock: C, inference: ComponentStatus) -> Self {
        Self {
            jobs,
            clock,
            inference,
        }
    }

    /// Collects the current database-backed operational status.
    pub fn execute(&self) -> Result<ApplicationStatus, StatusError> {
        let count = |state| {
            self.jobs
                .list_by_state(state)
                .map(|jobs| jobs.len() as u64)
                .map_err(|error| StatusError::probe(StatusComponent::Jobs, error))
        };
        let jobs = JobCounts::new(
            count(JobState::Pending)?,
            count(JobState::Running)?,
            count(JobState::Completed)?,
            count(JobState::Failed)?,
        );
        Ok(ApplicationStatus::new(
            self.clock.now(),
            ComponentStatus::Ready,
            jobs,
            self.inference.clone(),
        ))
    }
}

impl From<JobRepositoryError> for StatusError {
    fn from(error: JobRepositoryError) -> Self {
        Self::probe(StatusComponent::Jobs, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_depends_on_database_not_inference() {
        let status = ApplicationStatus::new(
            Timestamp::from_unix_millis(42),
            ComponentStatus::Ready,
            JobCounts::new(1, 2, 3, 4),
            ComponentStatus::Unavailable("Ollama is offline".to_owned()),
        );

        assert!(status.ready());
        assert_eq!(status.checked_at().as_unix_millis(), 42);
        assert_eq!(status.jobs().total(), 10);
        assert_eq!(status.inference().reason(), Some("Ollama is offline"));
    }

    #[test]
    fn unavailable_database_makes_status_not_ready() {
        let status = ApplicationStatus::new(
            Timestamp::from_unix_millis(7),
            ComponentStatus::unavailable("database locked"),
            JobCounts::default(),
            ComponentStatus::Ready,
        );

        assert!(!status.ready());
        assert_eq!(status.database().reason(), Some("database locked"));
    }

    #[test]
    fn status_errors_name_the_failed_component() {
        let error = StatusError::probe(StatusComponent::Jobs, "queue unavailable");
        assert_eq!(
            error.to_string(),
            "jobs status probe failed: queue unavailable"
        );
    }
}
