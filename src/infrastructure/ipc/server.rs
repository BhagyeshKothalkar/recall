//! Unix-domain-socket daemon transport.
//!
//! The server translates wire requests into application inputs and dispatches
//! them. It does not implement store, retrieval, or inference semantics.

use std::{fmt, io, os::unix::net::{UnixListener, UnixStream}, path::Path};

use serde::{de::DeserializeOwned, Serialize};

use crate::application::{ask::{AskError, AskRequest, AskResult}, store::{StoreInput, StoreError}};
use crate::runtime::composition::{AskService, StoreService};

use super::protocol::{RemoteError, RemoteErrorKind, Request, Response, StoreInput as WireStoreInput};

/// Maximum size of one framed IPC message.
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Errors produced by the Unix-domain-socket server.
#[derive(Debug)]
pub enum IpcServerError {
    /// Socket setup or transport failed.
    Io(io::Error),
    /// JSON framing or serialization failed.
    Serialization(String),
}

impl fmt::Display for IpcServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "IPC I/O error: {error}"),
            Self::Serialization(error) => write!(formatter, "IPC serialization error: {error}"),
        }
    }
}

impl std::error::Error for IpcServerError {}

impl From<io::Error> for IpcServerError {
    fn from(error: io::Error) -> Self { Self::Io(error) }
}

/// Server that dispatches supported requests to application services.
pub struct IpcServer<'a> {
    store: &'a StoreService,
    ask: &'a AskService,
}

impl<'a> IpcServer<'a> {
    /// Creates a transport server over the daemon's application services.
    pub const fn new(store: &'a StoreService, ask: &'a AskService) -> Self {
        Self { store, ask }
    }

    /// Binds the Unix socket and serves requests until the listener fails.
    pub fn serve(self, socket_path: &Path) -> Result<(), IpcServerError> {
        remove_stale_socket(socket_path)?;
        let listener = UnixListener::bind(socket_path)?;

        for stream in listener.incoming() {
            let stream = stream?;
            if let Err(error) = self.handle_connection(stream) {
                eprintln!("recall IPC request failed: {error}");
            }
        }

        Ok(())
    }

    fn handle_connection(&self, mut stream: UnixStream) -> Result<(), IpcServerError> {
        let request: Request = read_json(&mut stream)?;
        let response = self.dispatch(request);
        write_json(&mut stream, &response)
    }

    fn dispatch(&self, request: Request) -> Response {
        match request {
            Request::Store(request) => match to_store_input(request.input) {
                Ok(input) => match self.store.execute(input) {
                    Ok(result) => Response::Store(super::protocol::StoreResponse {
                        memory_id: result.memory_id().to_string(),
                        content_length: result.content_length(),
                    }),
                    Err(error) => Response::Error(RemoteError {
                        kind: classify_store_error(&error),
                        message: error.to_string(),
                    }),
                },
                Err(error) => Response::Error(error),
            },
            Request::Ask(request) => match AskRequest::new(request.question, request.use_ai) {
                Ok(request) => match self.ask.execute(request) {
                    Ok(AskResult::Answer(answer)) => Response::Answer(super::protocol::AnswerResponse {
                        text: answer.text().to_owned(),
                        sources: answer.sources().iter().map(ToString::to_string).collect(),
                    }),
                    Ok(AskResult::Retrieved(results)) => Response::Retrieved(super::protocol::RetrievedResponse {
                        memories: results.into_iter().map(|result| super::protocol::RetrievedMemoryResponse {
                            memory_id: result.memory_id().to_string(),
                            content: result.memory().content().to_owned(),
                            score: result.score().value(),
                        }).collect(),
                    }),
                    Err(error) => Response::Error(classify_ask_error(&error)),
                },
                Err(error) => Response::Error(RemoteError {
                    kind: RemoteErrorKind::Validation,
                    message: error.to_string(),
                }),
            },
            Request::Status(_) => unsupported("status is not implemented yet"),
        }
    }
}

fn to_store_input(input: WireStoreInput) -> Result<StoreInput, RemoteError> {
    match input {
        WireStoreInput::Text(value) => Ok(StoreInput::Text(value)),
        WireStoreInput::File(path) => Ok(StoreInput::File(path)),
    }
}

fn classify_store_error(error: &StoreError) -> RemoteErrorKind {
    match error {
        StoreError::EmptyContent | StoreError::ContentTooLarge { .. } | StoreError::InvalidMemory(_) => RemoteErrorKind::Validation,
        StoreError::ReadFile { .. } => RemoteErrorKind::Validation,
        StoreError::Repository(_) => RemoteErrorKind::Persistence,
    }
}

fn classify_ask_error(error: &AskError) -> RemoteError {
    let kind = match error {
        AskError::InvalidRequest(_) | AskError::InvalidGenerationRequest | AskError::InvalidRelevance => RemoteErrorKind::Validation,
        AskError::Search(_) => RemoteErrorKind::Search,
        AskError::Inference(_) => RemoteErrorKind::Inference,
    };
    RemoteError { kind, message: error.to_string() }
}

fn unsupported(message: &str) -> Response {
    Response::Error(RemoteError {
        kind: RemoteErrorKind::Protocol,
        message: message.to_owned(),
    })
}

fn remove_stale_socket(path: &Path) -> Result<(), IpcServerError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(IpcServerError::Io(error)),
    }
}

fn read_json<T: DeserializeOwned>(stream: &mut UnixStream) -> Result<T, IpcServerError> {
    use std::io::Read;
    let mut bytes = Vec::new();
    stream.take((MAX_MESSAGE_BYTES + 1) as u64).read_to_end(&mut bytes).map_err(IpcServerError::Io)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(IpcServerError::Serialization("IPC message exceeds maximum size".to_owned()));
    }
    serde_json::from_slice(&bytes).map_err(|error| IpcServerError::Serialization(error.to_string()))
}

fn write_json<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<(), IpcServerError> {
    use std::io::Write;
    let bytes = serde_json::to_vec(value).map_err(|error| IpcServerError::Serialization(error.to_string()))?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(IpcServerError::Serialization("IPC response exceeds maximum size".to_owned()));
    }
    stream.write_all(&bytes).map_err(IpcServerError::Io)
}
