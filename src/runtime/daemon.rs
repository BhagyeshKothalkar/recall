//! Recall daemon lifecycle.

use std::path::PathBuf;

use crate::infrastructure::ipc::server::{IpcServer, IpcServerError};
use crate::runtime::composition::{AskService, CompositionError, StoreService, build_ask_service, build_store_service};

/// Configuration required by the daemon runtime.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// SQLite database path owned by the daemon.
    pub database_path: PathBuf,
    /// Unix-domain socket path used by Recall clients.
    pub socket_path: PathBuf,
}

/// Long-lived Recall daemon.
///
/// The daemon owns the application services and their concrete infrastructure.
/// The CLI never receives direct access to the database or inference backend.
pub struct Daemon {
    config: DaemonConfig,
    store: StoreService,
    ask: AskService,
}

impl Daemon {
    /// Constructs the daemon and its concrete application dependencies.
    pub fn build(config: DaemonConfig) -> Result<Self, CompositionError> {
        let store = build_store_service(&config.database_path)?;
        let ask = build_ask_service(&config.database_path)?;
        Ok(Self { config, store, ask })
    }

    /// Returns the daemon's configured socket path.
    pub fn socket_path(&self) -> &std::path::Path {
        &self.config.socket_path
    }

    /// Runs the IPC server until the listener terminates.
    pub fn run(self) -> Result<(), IpcServerError> {
        let server = IpcServer::new(&self.store, &self.ask);
        server.serve(&self.config.socket_path)
    }
}
