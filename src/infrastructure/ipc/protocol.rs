//! Internal Unix-domain-socket request/response protocol.
//!
//! The wire model is separate from application and domain structs so the
//! transport can evolve without making the domain depend on serialization.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Maximum serialized size of one IPC request or response.
///
/// This is intentionally larger than the 16 MiB canonical-memory limit so
/// JSON framing and request metadata do not reject a valid maximum-sized
/// memory before the application can validate it.
pub const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

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
    /// Request a graceful daemon shutdown.
    Stop(StopRequest),
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

/// Stop request.
#[derive(Debug, Deserialize, Serialize)]
pub struct StopRequest;

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
    /// Successful shutdown request.
    Stopped,
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

#[cfg(test)]
mod tests {
    use super::{Response, RetrievedMemoryResponse, RetrievedResponse};

    #[test]
    fn retrieval_response_serializes_structured_memory_fields() {
        let response = Response::Retrieved(RetrievedResponse {
            memories: vec![RetrievedMemoryResponse {
                memory_id: "memory-1".to_owned(),
                content: "Rust uses ownership.".to_owned(),
                score: 0.75,
            }],
        });

        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("\"type\":\"Retrieved\""));
        assert!(json.contains("\"memory_id\":\"memory-1\""));
        assert!(json.contains("\"content\":\"Rust uses ownership.\""));
        assert!(json.contains("\"score\":0.75"));
    }
}

/// Current daemon status.
#[derive(Debug, Deserialize, Serialize)]
pub struct StatusResponse {
    /// Whether the daemon is ready to serve requests.
    pub ready: bool,
    /// Time at which this status was produced.
    pub checked_at_unix_millis: i64,
    /// Number of pending derivation jobs.
    pub pending_jobs: u64,
    /// Number of running derivation jobs.
    pub running_jobs: u64,
    /// Number of completed derivation jobs.
    pub completed_jobs: u64,
    /// Number of failed derivation jobs.
    pub failed_jobs: u64,
    /// Whether the configured inference backend is available.
    pub inference_ready: bool,
    /// Optional inference diagnostic.
    pub inference_error: Option<String>,
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
