//! Durable background derivation worker.

use std::{fmt, thread, time::Duration};

use crate::{
    application::ports::{Clock, EmbeddingRepository, InferenceBackend, JobRepository, MemoryRepository},
    domain::{EmbeddingRequest, InferenceModel, JobKind},
};

/// Executes persisted embedding jobs using the configured inference backend.
pub struct EmbeddingWorker<J, M, R, E, C> {
    jobs: J,
    memories: M,
    embeddings: R,
    inference: E,
    clock: C,
    model: InferenceModel,
}

impl<J, M, R, E, C> EmbeddingWorker<J, M, R, E, C>
where
    J: JobRepository,
    M: MemoryRepository,
    R: EmbeddingRepository,
    E: InferenceBackend,
    C: Clock,
{
    /// Constructs a worker from its explicit infrastructure capabilities.
    pub fn new(jobs: J, memories: M, embeddings: R, inference: E, clock: C, model: InferenceModel) -> Self {
        Self { jobs, memories, embeddings, inference, clock, model }
    }

    /// Processes at most one pending job.
    pub fn run_once(&self) -> Result<bool, WorkerError> {
        let Some(mut job) = self.jobs.claim_pending(self.clock.now())? else { return Ok(false); };
        let result = match job.kind() {
            JobKind::GenerateEmbedding { memory_id } => self.generate(memory_id),
        };
        match result {
            Ok(()) => {
                job.complete(self.clock.now()).map_err(WorkerError::Transition)?;
                self.jobs.update(&job)?;
                Ok(true)
            }
            Err(error) => {
                let message = error.to_string();
                job.fail(self.clock.now(), message.clone()).map_err(WorkerError::Transition)?;
                self.jobs.update(&job)?;
                Ok(true)
            }
        }
    }

    /// Runs the worker continuously with a small bounded polling interval.
    pub fn run_forever(&self) {
        loop {
            if let Err(error) = self.run_once() { eprintln!("recall embedding worker: {error}"); }
            thread::sleep(Duration::from_millis(250));
        }
    }

    fn generate(&self, memory_id: crate::domain::MemoryId) -> Result<(), WorkerError> {
        let memory = self.memories.get(memory_id)?.ok_or(WorkerError::MemoryMissing(memory_id))?;
        let request = EmbeddingRequest::new(memory.id(), memory.content().to_owned(), self.model.clone())
            .ok_or(WorkerError::InvalidEmbeddingRequest)?;
        let embedding = self.inference.embed(&request).map_err(WorkerError::Inference)?;
        self.embeddings.upsert(&embedding).map_err(WorkerError::EmbeddingRepository)
    }
}

/// Failure while executing one derivation job.
#[derive(Debug)]
pub enum WorkerError {
    /// Durable job persistence failed.
    Job(crate::application::ports::JobRepositoryError),
    /// Canonical memory persistence failed.
    Memory(crate::application::ports::MemoryRepositoryError),
    /// Expected canonical memory was missing.
    MemoryMissing(crate::domain::MemoryId),
    /// The domain rejected the embedding request.
    InvalidEmbeddingRequest,
    /// Inference failed.
    Inference(crate::application::ports::InferenceError),
    /// Derived embedding persistence failed.
    EmbeddingRepository(crate::application::ports::EmbeddingRepositoryError),
    /// A domain job transition was invalid.
    Transition(crate::domain::InvalidJobTransition),
}

impl fmt::Display for WorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Job(e) => write!(f, "job repository error: {e}"),
            Self::Memory(e) => write!(f, "memory repository error: {e}"),
            Self::MemoryMissing(id) => write!(f, "memory {id} no longer exists"),
            Self::InvalidEmbeddingRequest => f.write_str("could not construct embedding request"),
            Self::Inference(e) => write!(f, "embedding inference failed: {e}"),
            Self::EmbeddingRepository(e) => write!(f, "embedding repository error: {e}"),
            Self::Transition(e) => write!(f, "job transition failed: {e}"),
        }
    }
}

impl std::error::Error for WorkerError {}
impl From<crate::application::ports::JobRepositoryError> for WorkerError { fn from(e: crate::application::ports::JobRepositoryError) -> Self { Self::Job(e) } }
impl From<crate::application::ports::MemoryRepositoryError> for WorkerError { fn from(e: crate::application::ports::MemoryRepositoryError) -> Self { Self::Memory(e) } }
