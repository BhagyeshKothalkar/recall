//! Conversion from CLI arguments into application/IPC inputs.

use std::path::PathBuf;

use crate::infrastructure::ipc::protocol::StoreInput;

/// Converts the `store` argument according to Recall's initial UX rule:
/// an existing path is a file input; otherwise the value is literal text.
pub fn store_input(argument: String) -> StoreInput {
    let path = PathBuf::from(&argument);
    if path.is_file() {
        StoreInput::File(path)
    } else {
        StoreInput::Text(argument)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_path_becomes_file_input() {
        let path = std::env::temp_dir().join(format!("recall-cli-test-{}", std::process::id()));
        std::fs::write(&path, "hello").unwrap();

        assert_eq!(store_input(path.to_string_lossy().into_owned()), StoreInput::File(path.clone()));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn missing_path_is_literal_text() {
        let value = "this is not a file".to_owned();
        assert_eq!(store_input(value.clone()), StoreInput::Text(value));
    }
}
