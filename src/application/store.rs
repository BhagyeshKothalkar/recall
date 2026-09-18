//! Canonical memory store use case.

use std::{fmt, fs, path::PathBuf};

use super::ports::{Clock, StorePersistence, StorePersistenceError};
use crate::domain::{Job, JobKind, Memory, MemoryId, MemorySource};

/// Input supplied to the store use case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreInput {
    /// Store the supplied value literally as memory text.
    Text(String),
    /// Read an existing file and store its contents as memory text.
    File(PathBuf),
}

/// Memory-size constraints applied before persistence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryLimits {
    /// Maximum UTF-8 content size accepted for one canonical memory.
    pub max_content_bytes: usize,
}

impl MemoryLimits {
    /// Creates explicit memory limits.
    pub const fn new(max_content_bytes: usize) -> Self {
        Self { max_content_bytes }
    }
}

/// Successful result of canonical memory storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreResponse {
    memory_id: MemoryId,
    content_length: usize,
}

impl StoreResponse {
    /// Returns the identity assigned to the newly stored memory.
    pub const fn memory_id(&self) -> MemoryId {
        self.memory_id
    }

    /// Returns the content length in bytes.
    pub const fn content_length(&self) -> usize {
        self.content_length
    }
}

/// Errors raised while capturing or persisting a memory.
#[derive(Debug)]
pub enum StoreError {
    /// Input text was empty or whitespace-only.
    EmptyContent,
    /// Input exceeded the configured maximum size.
    ContentTooLarge { actual: usize, maximum: usize },
    /// A requested file could not be read.
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Canonical memory construction failed its domain invariants.
    InvalidMemory(crate::domain::MemoryValidationError),
    /// Canonical memory and derivation-job persistence failed.
    Persistence(StorePersistenceError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyContent => formatter.write_str("memory content cannot be empty"),
            Self::ContentTooLarge { actual, maximum } => write!(
                formatter,
                "memory content is {actual} bytes; maximum is {maximum} bytes"
            ),
            Self::ReadFile { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::InvalidMemory(error) => write!(formatter, "invalid memory: {error}"),
            Self::Persistence(error) => write!(formatter, "failed to persist memory: {error}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<StorePersistenceError> for StoreError {
    fn from(error: StorePersistenceError) -> Self {
        Self::Persistence(error)
    }
}

/// Application use case for canonical memory capture and persistence.
///
pub struct StoreMemory<P, C> {
    persistence: P,
    clock: C,
    limits: MemoryLimits,
}

impl<P, C> StoreMemory<P, C>
where
    P: StorePersistence,
    C: Clock,
{
    /// Creates a store use case with atomic persistence, clock, and limits.
    pub const fn new(persistence: P, clock: C, limits: MemoryLimits) -> Self {
        Self {
            persistence,
            clock,
            limits,
        }
    }

    /// Captures input, constructs canonical memory, and persists it.
    pub fn execute(&self, input: StoreInput) -> Result<StoreResponse, StoreError> {
        let (content, source) = capture(input)?;
        validate_size(&content, self.limits)?;

        let now = self.clock.now();
        let memory = Memory::new(MemoryId::new(), content, source, now, now)
            .map_err(StoreError::InvalidMemory)?;
        let job = Job::new(
            crate::domain::JobId::new(),
            JobKind::GenerateEmbedding {
                memory_id: memory.id(),
            },
            now,
        );
        let response = StoreResponse {
            memory_id: memory.id(),
            content_length: memory.content().len(),
        };

        self.persistence.persist(&memory, &job)?;
        Ok(response)
    }
}

fn capture(input: StoreInput) -> Result<(String, MemorySource), StoreError> {
    match input {
        StoreInput::Text(content) => Ok((content, MemorySource::DirectInput)),
        StoreInput::File(path) => {
            let content = fs::read_to_string(&path).map_err(|source| StoreError::ReadFile {
                path: path.clone(),
                source,
            })?;
            Ok((content, MemorySource::File { path }))
        }
    }
}

fn validate_size(content: &str, limits: MemoryLimits) -> Result<(), StoreError> {
    if content.trim().is_empty() {
        return Err(StoreError::EmptyContent);
    }

    let actual = content.len();
    if actual > limits.max_content_bytes {
        return Err(StoreError::ContentTooLarge {
            actual,
            maximum: limits.max_content_bytes,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap};

    use super::*;
    use crate::domain::Timestamp;

    struct FixedClock(Timestamp);

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    #[derive(Default)]
    struct MemoryStore {
        memories: RefCell<HashMap<MemoryId, Memory>>,
        jobs: RefCell<Vec<Job>>,
    }

    impl StorePersistence for &MemoryStore {
        fn persist(&self, memory: &Memory, job: &Job) -> Result<(), StorePersistenceError> {
            self.memories
                .borrow_mut()
                .insert(memory.id(), memory.clone());
            self.jobs.borrow_mut().push(job.clone());
            Ok(())
        }
    }

    fn use_case(store: &MemoryStore) -> StoreMemory<&MemoryStore, FixedClock> {
        StoreMemory::new(
            store,
            FixedClock(Timestamp::from_unix_millis(42)),
            MemoryLimits::new(1024),
        )
    }

    #[test]
    fn stores_direct_text() {
        let store = MemoryStore::default();
        let response = use_case(&store)
            .execute(StoreInput::Text("hello".to_owned()))
            .unwrap();

        let memory = store
            .memories
            .borrow()
            .get(&response.memory_id())
            .cloned()
            .unwrap();
        assert_eq!(memory.content(), "hello");
        assert_eq!(memory.source(), &MemorySource::DirectInput);
        assert_eq!(response.content_length(), 5);
        assert_eq!(store.jobs.borrow().len(), 1);
        assert_eq!(
            store.jobs.borrow()[0].state(),
            crate::domain::JobState::Pending
        );
    }

    #[test]
    fn rejects_empty_text_before_persistence() {
        let store = MemoryStore::default();
        let result = use_case(&store).execute(StoreInput::Text(" \n".to_owned()));

        assert!(matches!(result, Err(StoreError::EmptyContent)));
        assert!(store.memories.borrow().is_empty());
    }

    #[test]
    fn rejects_content_over_limit_before_persistence() {
        let store = MemoryStore::default();
        let use_case = StoreMemory::new(
            &store,
            FixedClock(Timestamp::from_unix_millis(42)),
            MemoryLimits::new(4),
        );

        let result = use_case.execute(StoreInput::Text("hello".to_owned()));

        assert!(matches!(
            result,
            Err(StoreError::ContentTooLarge {
                actual: 5,
                maximum: 4
            })
        ));
        assert!(store.memories.borrow().is_empty());
    }
}
