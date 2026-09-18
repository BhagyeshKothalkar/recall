//! Recall daemon lifecycle.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
};

use crate::infrastructure::ipc::server::{IpcServer, IpcServerError};
use crate::runtime::composition::{
    build_ask_service, build_status_service, build_store_service, build_worker, AskService,
    CompositionError, StatusService, StoreService, WorkerService,
};

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
    status: StatusService,
    worker_stop: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
}

impl Daemon {
    /// Constructs the daemon and its concrete application dependencies.
    pub fn build(config: DaemonConfig) -> Result<Self, CompositionError> {
        create_parent(&config.database_path).map_err(CompositionError::Filesystem)?;
        create_parent(&config.socket_path).map_err(CompositionError::Filesystem)?;
        let store = build_store_service(&config.database_path)?;
        let ask = build_ask_service(&config.database_path)?;
        let status = build_status_service(&config.database_path)?;
        let worker: WorkerService = build_worker(&config.database_path)?;
        let worker_stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&worker_stop);
        let worker_handle = thread::spawn(move || worker.run_until(&stop_for_thread));
        Ok(Self {
            config,
            store,
            ask,
            status,
            worker_stop,
            worker_handle: Some(worker_handle),
        })
    }

    /// Returns the daemon's configured socket path.
    pub fn socket_path(&self) -> &std::path::Path {
        &self.config.socket_path
    }

    /// Runs the IPC server until the listener terminates.
    pub fn run(mut self) -> Result<(), IpcServerError> {
        let server = IpcServer::new(&self.store, &self.ask, &self.status);
        let result = server.serve(&self.config.socket_path);
        self.worker_stop.store(true, Ordering::Release);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
        result
    }
}

fn create_parent(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
