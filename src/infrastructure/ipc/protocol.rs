//! Internal Unix-domain-socket request/response protocol.
//!
//! The wire model is separate from application and domain structs so the
//! transport can evolve without making the domain depend on serialization.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Request sent from a Recall client to the daemon.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum Request {
    /// Store one piece of user-provided memory.
    Store(StoreRequest),
    /// Ask Recall to retrieve and optionally answer a question.
    Ask(AskRequest),
    /// Request daemon status.
    Status(StatusRequest),
}

/// Input crossing the IPC boundary for a store operation.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value")]
pub enum StoreInput {
    /// Store the supplied string literally.
    Text(String),
    /// Read and store the supplied file path.
    File(PathBuf),
}

/// Store request sent to the daemon.
#[derive(Debug, Deserialize, Serialize)]
pub struct StoreRequest {
    /// Input to canonical memory capture.
    pub input: StoreInput,
}

/// Ask request sent to the daemon.
#[derive(Debug, Deserialize, Serialize)]
pub struct AskRequest {
    /// User's question.
    pub question: String,
    /// Whether Recall should invoke the configured inference backend.
    pub use_ai: bool,
}

/// Status request.
#[derive(Debug, Deserialize, Serialize)]
pub struct StatusRequest;

/// Response returned by the daemon.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum Response {
    /// Successful store operation.
    Store(StoreResponse),
    /// Successful AI-backed answer.
    Answer(AnswerResponse),
    /// Successful retrieval-only operation.
    Retrieved(RetrievedResponse),
    /// Successful status operation.
    Status(StatusResponse),
    /// Application or runtime failure.
    Error(RemoteError),
}

/// Result of a successful store operation.
#[derive(Debug, Deserialize, Serialize)]
pub struct StoreResponse {
    /// Stable canonical memory identity.
    pub memory_id: String,
    /// Number of UTF-8 bytes in the stored content.
    pub content_length: usize,
}

/// Result of a successful answer operation.
#[derive(Debug, Deserialize, Serialize)]
pub struct AnswerResponse {
    /// Generated answer text.
    pub text: String,
    /// Provenance selected by the application.
    pub sources: Vec<String>,
}

/// Result of retrieval without AI generation.
#[derive(Debug, Deserialize, Serialize)]
pub struct RetrievedResponse {
    /// Memories selected by lexical retrieval, in relevance order.
    pub memories: Vec<RetrievedMemoryResponse>,
}

/// One retrieval-only memory rendered across IPC.
#[derive(Debug, Deserialize, Serialize)]
pub struct RetrievedMemoryResponse {
    /// Stable canonical memory identity.
    pub memory_id: String,
    /// Canonical memory content.
    pub content: String,
    /// Retrieval score assigned by the search implementation.
    pub score: f32,
}

/// Current daemon status.
#[derive(Debug, Deserialize, Serialize)]
pub struct StatusResponse {
    /// Whether the daemon is ready to serve requests.
    pub ready: bool,
    /// Time at which this status was produced.
    pub checked_at_unix_millis: i64,
}

/// Error represented on the internal wire protocol.
#[derive(Debug, Deserialize, Serialize)]
pub struct RemoteError {
    /// Stable category for client-side handling.
    pub kind: RemoteErrorKind,
    /// Human-readable diagnostic.
    pub message: String,
}

/// Coarse error categories crossing the internal protocol.
#[derive(Debug, Deserialize, Serialize)]
pub enum RemoteErrorKind {
    /// Request failed validation.
    Validation,
    /// Canonical persistence failed.
    Persistence,
    /// Retrieval failed.
    Search,
    /// Inference failed.
    Inference,
    /// Background derivation failed.
    Job,
    /// Daemon/runtime failure.
    Runtime,
    /// Request could not be decoded or is not supported.
    Protocol,
}
