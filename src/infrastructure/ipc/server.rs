//! Unix-domain-socket daemon transport.
//!
//! The server translates wire requests into application inputs and dispatches
//! them. It does not implement store, retrieval, or inference semantics.

use std::{
    fmt, io,
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use serde::{de::DeserializeOwned, Serialize};

use crate::application::{
    ask::{AskError, AskRequest},
    status::{ApplicationStatus, StatusError},
    store::{StoreError, StoreInput},
};

use crate::runtime::composition::{AskService, StatusService, StoreService};

use super::protocol::{
    RemoteError, RemoteErrorKind, Request, Response, StoreInput as WireStoreInput,
    MAX_MESSAGE_BYTES,
};

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
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Server that dispatches supported requests to application services.
pub struct IpcServer<'a> {
    store: &'a StoreService,
    ask: &'a AskService,
    status: &'a StatusService,
    stop: Arc<AtomicBool>,
}

impl<'a> IpcServer<'a> {
    /// Creates a transport server over the daemon's application services.
    pub fn new(store: &'a StoreService, ask: &'a AskService, status: &'a StatusService) -> Self {
        Self {
            store,
            ask,
            status,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Binds the Unix socket and serves requests until the listener fails.
    pub fn serve(self, socket_path: &Path) -> Result<(), IpcServerError> {
        remove_stale_socket(socket_path)?;
        let listener = UnixListener::bind(socket_path)?;
        listener.set_nonblocking(true)?;

        while !self.stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if let Err(error) = self.handle_connection(stream) {
                        eprintln!("recall IPC request failed: {error}");
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(IpcServerError::Io(error)),
            }
        }

        let _ = std::fs::remove_file(socket_path);
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
                    Ok(answer) => match answer.retrieved() {
                        Some(memories) => Response::Retrieved(super::protocol::RetrievedResponse {
                            memories: memories
                                .iter()
                                .map(|memory| super::protocol::RetrievedMemoryResponse {
                                    memory_id: memory.memory_id().to_string(),
                                    content: memory.content().to_owned(),
                                    score: memory.score(),
                                })
                                .collect(),
                        }),
                        None => Response::Answer(super::protocol::AnswerResponse {
                            text: answer.text().to_owned(),
                            sources: answer.sources().iter().map(ToString::to_string).collect(),
                        }),
                    },
                    Err(error) => Response::Error(classify_ask_error(&error)),
                },
                Err(error) => Response::Error(RemoteError {
                    kind: RemoteErrorKind::Validation,
                    message: error.to_string(),
                }),
            },
            Request::Status(_) => match self.status.execute() {
                Ok(status) => Response::Status(to_status_response(&status)),
                Err(error) => Response::Error(classify_status_error(&error)),
            },
            Request::Stop(_) => {
                self.stop.store(true, Ordering::Release);
                Response::Stopped
            }
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
        StoreError::EmptyContent
        | StoreError::ContentTooLarge { .. }
        | StoreError::InvalidMemory(_) => RemoteErrorKind::Validation,
        StoreError::ReadFile { .. } => RemoteErrorKind::Validation,
        StoreError::Persistence(_) => RemoteErrorKind::Persistence,
    }
}

fn classify_ask_error(error: &AskError) -> RemoteError {
    let kind = match error {
        AskError::InvalidRequest(_)
        | AskError::InvalidGenerationRequest
        | AskError::InvalidRelevance => RemoteErrorKind::Validation,
        AskError::Search(_) => RemoteErrorKind::Search,
        AskError::Inference(_) => RemoteErrorKind::Inference,
    };
    RemoteError {
        kind,
        message: error.to_string(),
    }
}

fn classify_status_error(error: &StatusError) -> RemoteError {
    RemoteError {
        kind: RemoteErrorKind::Runtime,
        message: error.to_string(),
    }
}

fn to_status_response(status: &ApplicationStatus) -> super::protocol::StatusResponse {
    let jobs = status.jobs();
    super::protocol::StatusResponse {
        ready: status.ready(),
        checked_at_unix_millis: status.checked_at().as_unix_millis(),
        pending_jobs: jobs.pending(),
        running_jobs: jobs.running(),
        completed_jobs: jobs.completed(),
        failed_jobs: jobs.failed(),
        inference_ready: status.inference().is_ready(),
        inference_error: status.inference().reason().map(ToOwned::to_owned),
    }
}

fn remove_stale_socket(path: &Path) -> Result<(), IpcServerError> {
    if !path.exists() {
        return Ok(());
    }

    match UnixStream::connect(path) {
        Ok(_) => Err(IpcServerError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Recall daemon is already running",
        ))),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) =>
        {
            match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(remove_error) if remove_error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(remove_error) => Err(IpcServerError::Io(remove_error)),
            }
        }
        Err(error) => Err(IpcServerError::Io(error)),
    }
}

fn read_json<T: DeserializeOwned>(stream: &mut UnixStream) -> Result<T, IpcServerError> {
    use std::io::Read;
    let mut bytes = Vec::new();
    stream
        .take((MAX_MESSAGE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(IpcServerError::Io)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(IpcServerError::Serialization(
            "IPC message exceeds maximum size".to_owned(),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| IpcServerError::Serialization(error.to_string()))
}

fn write_json<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<(), IpcServerError> {
    use std::io::Write;
    let bytes = serde_json::to_vec(value)
        .map_err(|error| IpcServerError::Serialization(error.to_string()))?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(IpcServerError::Serialization(
            "IPC response exceeds maximum size".to_owned(),
        ));
    }
    stream.write_all(&bytes).map_err(IpcServerError::Io)
}
