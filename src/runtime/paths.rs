//! Default filesystem locations for Recall runtime state.

use std::{
    env, io,
    path::{Path, PathBuf},
};

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
    pub fn default_paths() -> io::Result<Self> {
        let base = xdg_data_home()?;
        Ok(Self::from_data_dir(base.join("recall")))
    }

    /// Builds the standard database and socket locations below `data_dir`.
    ///
    /// This constructor is useful for callers that have already resolved an
    /// application data directory, and keeps the filename conventions in one
    /// place.
    pub fn from_data_dir(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        Self {
            database: data_dir.join("recall.db"),
            socket: data_dir.join("recall.sock"),
            data_dir,
        }
    }

    /// Resolves optional database and socket overrides against the defaults.
    ///
    /// An explicitly supplied path is preserved exactly, including whether it
    /// is relative or absolute. This makes path ownership explicit while
    /// allowing the daemon and clients to apply the same defaults.
    pub fn resolve(database: Option<&Path>, socket: Option<&Path>) -> io::Result<Self> {
        let defaults = Self::default_paths()?;
        Ok(Self {
            data_dir: defaults.data_dir,
            database: database.map(Path::to_path_buf).unwrap_or(defaults.database),
            socket: socket.map(Path::to_path_buf).unwrap_or(defaults.socket),
        })
    }

    /// Creates the parent directories required by the configured paths.
    ///
    /// This is deliberately an explicit operation: resolving paths is
    /// side-effect free, while runtime startup can opt into filesystem setup.
    pub fn create_parent_dirs(&self) -> io::Result<()> {
        if let Some(parent) = self.database.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.socket.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Directory containing Recall's persistent local state.
    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    /// Default SQLite database path.
    pub fn database(&self) -> &std::path::Path {
        &self.database
    }

    /// Returns the configured SQLite database path.
    pub fn database_path(&self) -> &Path {
        self.database()
    }

    /// Default Unix-domain socket path.
    pub fn socket(&self) -> &std::path::Path {
        &self.socket
    }

    /// Returns the configured Unix-domain socket path.
    pub fn socket_path(&self) -> &Path {
        self.socket()
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

#[cfg(test)]
mod tests {
    use super::RecallPaths;
    use std::path::Path;

    #[test]
    fn data_directory_controls_default_filenames() {
        let paths = RecallPaths::from_data_dir("/tmp/recall-test");

        assert_eq!(paths.data_dir(), Path::new("/tmp/recall-test"));
        assert_eq!(paths.database(), Path::new("/tmp/recall-test/recall.db"));
        assert_eq!(paths.socket(), Path::new("/tmp/recall-test/recall.sock"));
    }

    #[test]
    fn resolve_preserves_explicit_overrides_and_fills_missing_values() {
        let database = Path::new("custom/recall.db");
        let paths = RecallPaths::resolve(Some(database), None).unwrap();

        assert_eq!(paths.database(), Path::new("custom/recall.db"));
        assert!(paths.socket().ends_with("recall/recall.sock"));
    }

    #[test]
    fn parent_directory_creation_is_explicit() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("recall-paths-{unique}"));
        let paths = RecallPaths::resolve(
            Some(root.join("nested/db/recall.db").as_path()),
            Some(root.join("nested/socket/recall.sock").as_path()),
        )
        .unwrap();

        assert!(!root.exists());
        paths.create_parent_dirs().unwrap();
        assert!(root.join("nested/db").is_dir());
        assert!(root.join("nested/socket").is_dir());

        std::fs::remove_dir_all(root).unwrap();
    }
}
