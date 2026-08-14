//! Terminal rendering for daemon responses.

use crate::infrastructure::ipc::protocol::{RemoteError, Response};

/// Renders one daemon response for the normal CLI output path.
pub fn render(response: Response) -> Result<(), RenderError> {
    match response {
        Response::Store(result) => {
            println!("Stored memory {}", result.memory_id);
            Ok(())
        }
        Response::Answer(answer) => {
            println!("{}", answer.text);
            if !answer.sources.is_empty() {
                println!("\nSources:");
                for source in answer.sources {
                    println!("- {}", source);
                }
            }
            Ok(())
        }
        Response::Retrieved(result) => {
            if result.memories.is_empty() {
                println!("No matching memories.");
                return Ok(());
            }

            for (index, memory) in result.memories.iter().enumerate() {
                println!("{}. [{}]", index + 1, memory.memory_id);
                println!("{}", memory.content);
                println!();
            }
            Ok(())
        }
        Response::Status(status) => {
            println!("daemon: {}", if status.ready { "ready" } else { "not ready" });
            Ok(())
        }
        Response::Error(error) => Err(RenderError::Remote(error)),
    }
}

/// Rendering failures that should become a non-zero CLI exit.
#[derive(Debug)]
pub enum RenderError {
    /// The daemon returned an application/runtime failure.
    Remote(RemoteError),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Remote(error) => write!(formatter, "{}", error.message),
        }
    }
}

impl std::error::Error for RenderError {}
