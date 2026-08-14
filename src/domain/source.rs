//! Provenance of captured canonical memory.

use std::path::PathBuf;

/// Identifies how a memory entered Recall.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemorySource {
    /// Memory supplied directly as user text.
    DirectInput,
    /// Memory read from an explicitly supplied file path.
    File { path: PathBuf },
}

impl MemorySource {
    /// Returns the file path when this source originated from a file.
    pub fn file_path(&self) -> Option<&std::path::Path> {
        match self {
            Self::DirectInput => None,
            Self::File { path } => Some(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_input_has_no_file_path() {
        assert_eq!(MemorySource::DirectInput.file_path(), None);
    }

    #[test]
    fn file_source_preserves_path() {
        let source = MemorySource::File {
            path: PathBuf::from("notes.txt"),
        };

        assert_eq!(source.file_path(), Some(std::path::Path::new("notes.txt")));
    }
}
