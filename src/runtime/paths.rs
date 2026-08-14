//! Default filesystem locations for Recall runtime state.

use std::{env, io, path::PathBuf};

/// Centralized filesystem locations used by the local Recall runtime.
#[derive(Debug, Clone)]
pub struct RecallPaths {
    data_dir: PathBuf,
    database: PathBuf,
    socket: PathBuf,
}

impl RecallPaths {
    /// Resolves the default Recall data locations.
    ///
    /// `XDG_DATA_HOME` is honored when present; otherwise the standard Linux
    /// fallback is `~/.local/share`. Recall keeps its database in a dedicated
    /// `recall` directory rather than the project working directory.
    pub fn default() -> io::Result<Self> {
        let base = xdg_data_home()?;
        let data_dir = base.join("recall");
        Ok(Self {
            database: data_dir.join("recall.db"),
            socket: data_dir.join("recall.sock"),
            data_dir,
        })
    }

    /// Directory containing Recall's persistent local state.
    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    /// Default SQLite database path.
    pub fn database(&self) -> &std::path::Path {
        &self.database
    }

    /// Default Unix-domain socket path.
    pub fn socket(&self) -> &std::path::Path {
        &self.socket
    }
}

fn xdg_data_home() -> io::Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;

    Ok(PathBuf::from(home).join(".local").join("share"))
}
