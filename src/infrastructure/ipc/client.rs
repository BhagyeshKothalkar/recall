//! Unix-domain-socket IPC client used by CLI/runtime clients.

use std::{fmt, io, os::unix::net::UnixStream, path::Path};

use serde::{de::DeserializeOwned, Serialize};

use super::protocol::{Request, Response};

const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Errors produced while communicating with the daemon.
#[derive(Debug)]
pub enum IpcClientError {
    /// Socket transport failed.
    Io(io::Error),
    /// JSON encoding/decoding failed.
    Serialization(String),
}

impl fmt::Display for IpcClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "IPC I/O error: {error}"),
            Self::Serialization(error) => write!(formatter, "IPC serialization error: {error}"),
        }
    }
}

impl std::error::Error for IpcClientError {}

impl From<io::Error> for IpcClientError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Synchronous request/response client for the daemon socket.
pub struct IpcClient {
    stream: UnixStream,
}

impl IpcClient {
    /// Connects to an existing daemon socket.
    pub fn connect(socket_path: &Path) -> Result<Self, IpcClientError> {
        Ok(Self {
            stream: UnixStream::connect(socket_path)?,
        })
    }

    /// Sends one request and waits for its complete response.
    pub fn request(&self, request: Request) -> Result<Response, IpcClientError> {
        let mut writer = self.stream.try_clone()?;
        write_json(&mut writer, &request)?;
        drop(writer);

        let mut reader = self.stream.try_clone()?;
        read_json(&mut reader)
    }
}

fn read_json<T: DeserializeOwned>(stream: &mut UnixStream) -> Result<T, IpcClientError> {
    use std::io::Read;
    let mut bytes = Vec::new();
    stream
        .take((MAX_MESSAGE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(IpcClientError::Serialization(
            "IPC message exceeds maximum size".to_owned(),
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| IpcClientError::Serialization(error.to_string()))
}

fn write_json<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<(), IpcClientError> {
    use std::io::Write;
    let bytes = serde_json::to_vec(value)
        .map_err(|error| IpcClientError::Serialization(error.to_string()))?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(IpcClientError::Serialization(
            "IPC request exceeds maximum size".to_owned(),
        ));
    }
    stream.write_all(&bytes)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    Ok(())
}
