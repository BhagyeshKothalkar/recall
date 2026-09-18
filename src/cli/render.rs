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
            print!("{}", format_retrieved(&result));
            Ok(())
        }
        Response::Status(status) => {
            println!(
                "daemon: {}",
                if status.ready { "ready" } else { "not ready" }
            );
            println!(
                "jobs: pending={}, running={}, completed={}, failed={}",
                status.pending_jobs, status.running_jobs, status.completed_jobs, status.failed_jobs
            );
            println!(
                "inference: {}",
                if status.inference_ready {
                    "ready"
                } else {
                    "unavailable"
                }
            );
            Ok(())
        }
        Response::Stopped => {
            println!("daemon: stopping");
            Ok(())
        }
        Response::Error(error) => Err(RenderError::Remote(error)),
    }
}

fn format_retrieved(result: &crate::infrastructure::ipc::protocol::RetrievedResponse) -> String {
    if result.memories.is_empty() {
        return "No matching memories.\n".to_owned();
    }

    result
        .memories
        .iter()
        .enumerate()
        .map(|(index, memory)| {
            format!(
                "{}. [{}] (score: {:.3})\n{}\n\n",
                index + 1,
                memory.memory_id,
                memory.score,
                memory.content
            )
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::format_retrieved;
    use crate::infrastructure::ipc::protocol::{RetrievedMemoryResponse, RetrievedResponse};

    #[test]
    fn retrieval_render_includes_id_content_and_score() {
        let output = format_retrieved(&RetrievedResponse {
            memories: vec![RetrievedMemoryResponse {
                memory_id: "memory-1".to_owned(),
                content: "Rust uses ownership.".to_owned(),
                score: 0.75,
            }],
        });

        assert!(output.contains("[memory-1]"));
        assert!(output.contains("score: 0.750"));
        assert!(output.contains("Rust uses ownership."));
    }

    #[test]
    fn empty_retrieval_render_is_explicit() {
        assert_eq!(
            format_retrieved(&RetrievedResponse { memories: vec![] }),
            "No matching memories.\n"
        );
    }
}
