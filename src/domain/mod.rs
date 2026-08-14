//! Recall's backend-independent domain model.
//!
//! The domain contains canonical memory state and the value types used to
//! describe retrieval, inference, and durable derivation work. It has no
//! knowledge of SQLite, Ollama, IPC, CLI parsing, or other infrastructure.

mod inference;
mod job;
mod memory;
mod search;
mod source;

pub use inference::{
    EmbeddingRequest, EmbeddingResponse, GenerationRequest, GenerationResponse, InferenceModel,
};
pub use job::{InvalidJobTransition, Job, JobId, JobKind, JobState};
pub use memory::{Memory, MemoryId, MemoryValidationError, Timestamp};
pub use search::{Relevance, RetrievedMemory, SearchQuery, SearchQueryError, SearchResult, SearchScore};
pub use source::MemorySource;
