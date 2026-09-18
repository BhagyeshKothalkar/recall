//! Durable background derivation worker.

use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use crate::{
    application::ports::{
        Clock, EmbeddingRepository, InferenceBackend, JobRepository, MemoryRepository,
    },
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
    pub fn new(
        jobs: J,
        memories: M,
        embeddings: R,
        inference: E,
        clock: C,
        model: InferenceModel,
    ) -> Self {
        Self {
            jobs,
            memories,
            embeddings,
            inference,
            clock,
            model,
        }
    }

    /// Processes at most one pending job.
    pub fn run_once(&self) -> Result<bool, WorkerError> {
        let Some(mut job) = self.jobs.claim_pending(self.clock.now())? else {
            return Ok(false);
        };
        let result = match job.kind() {
            JobKind::GenerateEmbedding { memory_id } => self.generate(memory_id),
        };
        match result {
            Ok(()) => {
                job.complete(self.clock.now())
                    .map_err(WorkerError::Transition)?;
                self.jobs.update(&job)?;
                Ok(true)
            }
            Err(error) => {
                let message = error.to_string();
                job.fail(self.clock.now(), message.clone())
                    .map_err(WorkerError::Transition)?;
                self.jobs.update(&job)?;
                Ok(true)
            }
        }
    }

    /// Runs the worker continuously with a small bounded polling interval.
    pub fn run_forever(&self) {
        let stop = AtomicBool::new(false);
        self.run_until(&stop);
    }

    /// Runs until the supplied shutdown flag is set.
    pub fn run_until(&self, stop: &AtomicBool) {
        while !stop.load(Ordering::Acquire) {
            if let Err(error) = self.run_once() {
                eprintln!("recall embedding worker: {error}");
            }
            if !stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(250));
            }
        }
    }

    fn generate(&self, memory_id: crate::domain::MemoryId) -> Result<(), WorkerError> {
        let memory = self
            .memories
            .get(memory_id)?
            .ok_or(WorkerError::MemoryMissing(memory_id))?;
        let request =
            EmbeddingRequest::new(memory.id(), memory.content().to_owned(), self.model.clone())
                .ok_or(WorkerError::InvalidEmbeddingRequest)?;
        let embedding = self
            .inference
            .embed(&request)
            .map_err(WorkerError::Inference)?;
        self.embeddings
            .upsert(&embedding)
            .map_err(WorkerError::EmbeddingRepository)
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
impl From<crate::application::ports::JobRepositoryError> for WorkerError {
    fn from(e: crate::application::ports::JobRepositoryError) -> Self {
        Self::Job(e)
    }
}
impl From<crate::application::ports::MemoryRepositoryError> for WorkerError {
    fn from(e: crate::application::ports::MemoryRepositoryError) -> Self {
        Self::Memory(e)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::{
        application::ports::{
            EmbeddingRepositoryError, JobRepository, JobRepositoryError, MemoryRepositoryError,
        },
        domain::{
            EmbeddingResponse, GenerationRequest, GenerationResponse, Job, JobId, JobState, Memory,
            MemoryId, MemorySource, Timestamp,
        },
    };

    #[derive(Clone, Copy)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            Timestamp::from_unix_millis(20)
        }
    }

    struct FakeJobs {
        job: RefCell<Job>,
    }

    impl JobRepository for &FakeJobs {
        fn create(&self, job: &Job) -> Result<(), JobRepositoryError> {
            self.job.replace(job.clone());
            Ok(())
        }

        fn claim_pending(&self, now: Timestamp) -> Result<Option<Job>, JobRepositoryError> {
            let mut job = self.job.borrow_mut();
            if job.state() != JobState::Pending {
                return Ok(None);
            }
            job.start(now).map_err(|error| {
                JobRepositoryError::Storage(Box::new(std::io::Error::other(error.to_string())))
            })?;
            Ok(Some(job.clone()))
        }

        fn update(&self, job: &Job) -> Result<(), JobRepositoryError> {
            self.job.replace(job.clone());
            Ok(())
        }

        fn get(&self, id: JobId) -> Result<Option<Job>, JobRepositoryError> {
            Ok((self.job.borrow().id() == id).then(|| self.job.borrow().clone()))
        }

        fn list_by_state(&self, state: JobState) -> Result<Vec<Job>, JobRepositoryError> {
            Ok((self.job.borrow().state() == state)
                .then(|| self.job.borrow().clone())
                .into_iter()
                .collect())
        }
    }

    struct FakeMemories {
        memory: Memory,
    }

    impl MemoryRepository for &FakeMemories {
        fn create(&self, _memory: &Memory) -> Result<(), MemoryRepositoryError> {
            Ok(())
        }

        fn get(&self, id: MemoryId) -> Result<Option<Memory>, MemoryRepositoryError> {
            Ok((self.memory.id() == id).then(|| self.memory.clone()))
        }

        fn delete(&self, _id: MemoryId) -> Result<(), MemoryRepositoryError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeEmbeddings {
        value: RefCell<Option<EmbeddingResponse>>,
    }

    impl EmbeddingRepository for &FakeEmbeddings {
        fn upsert(&self, embedding: &EmbeddingResponse) -> Result<(), EmbeddingRepositoryError> {
            self.value.replace(Some(embedding.clone()));
            Ok(())
        }

        fn delete(&self, _memory_id: MemoryId) -> Result<(), EmbeddingRepositoryError> {
            self.value.replace(None);
            Ok(())
        }

        fn exists_for_model(
            &self,
            _memory_id: MemoryId,
            _model: &InferenceModel,
        ) -> Result<bool, EmbeddingRepositoryError> {
            Ok(self.value.borrow().is_some())
        }
    }

    struct FakeInference {
        fail: bool,
    }

    impl InferenceBackend for &FakeInference {
        fn generate(
            &self,
            _request: &GenerationRequest,
        ) -> Result<GenerationResponse, crate::application::ports::InferenceError> {
            unreachable!("worker does not generate")
        }

        fn embed(
            &self,
            request: &EmbeddingRequest,
        ) -> Result<EmbeddingResponse, crate::application::ports::InferenceError> {
            if self.fail {
                return Err(crate::application::ports::InferenceError::InvalidResponse(
                    "offline".to_owned(),
                ));
            }
            Ok(
                EmbeddingResponse::new(
                    request.memory_id(),
                    request.model().clone(),
                    vec![1.0, 0.0],
                )
                .unwrap(),
            )
        }
    }

    fn fixtures(
        fail: bool,
    ) -> (
        FakeJobs,
        FakeMemories,
        FakeEmbeddings,
        FakeInference,
        MemoryId,
    ) {
        let timestamp = Timestamp::from_unix_millis(10);
        let memory = Memory::new(
            MemoryId::new(),
            "Rust ownership".to_owned(),
            MemorySource::DirectInput,
            timestamp,
            timestamp,
        )
        .unwrap();
        let memory_id = memory.id();
        (
            FakeJobs {
                job: RefCell::new(Job::new(
                    JobId::new(),
                    JobKind::GenerateEmbedding { memory_id },
                    timestamp,
                )),
            },
            FakeMemories { memory },
            FakeEmbeddings::default(),
            FakeInference { fail },
            memory_id,
        )
    }

    #[test]
    fn successful_job_persists_embedding_and_completes() {
        let (jobs, memories, embeddings, inference, memory_id) = fixtures(false);
        let worker = EmbeddingWorker::new(
            &jobs,
            &memories,
            &embeddings,
            &inference,
            FixedClock,
            InferenceModel::new("test".to_owned()).unwrap(),
        );

        assert!(worker.run_once().unwrap());
        assert_eq!(jobs.job.borrow().state(), JobState::Completed);
        assert_eq!(
            embeddings.value.borrow().as_ref().unwrap().memory_id(),
            memory_id
        );
    }

    #[test]
    fn failed_embedding_marks_job_without_deleting_memory() {
        let (jobs, memories, embeddings, inference, memory_id) = fixtures(true);
        let worker = EmbeddingWorker::new(
            &jobs,
            &memories,
            &embeddings,
            &inference,
            FixedClock,
            InferenceModel::new("test".to_owned()).unwrap(),
        );

        assert!(worker.run_once().unwrap());
        assert_eq!(jobs.job.borrow().state(), JobState::Failed);
        assert!(MemoryRepository::get(&&memories, memory_id)
            .unwrap()
            .is_some());
        assert!(embeddings.value.borrow().is_none());
    }
}
