//! CLI orchestration.

pub mod command;
pub mod input;
pub mod render;

use std::path::Path;

use command::{Cli, Command};
use crate::infrastructure::ipc::client::{IpcClient, IpcClientError};
use crate::infrastructure::ipc::protocol::{Request, StoreRequest};
use crate::runtime::daemon::{Daemon, DaemonConfig};

/// Runs the parsed CLI command.
pub fn run(cli: Cli) -> Result<(), CliError> {
    let no_ai = cli.no_ai;
    match (cli.question, cli.command) {
        (Some(question), None) => {
            let client = IpcClient::connect(Path::new("recall.sock"))?;
            let response = client.request(Request::Ask(
                crate::infrastructure::ipc::protocol::AskRequest {
                    question,
                    use_ai: !no_ai,
                },
            ))?;
            render::render(response).map_err(CliError::Render)
        }
        (None, Some(Command::Store(args))) => {
            let client = IpcClient::connect(Path::new("recall.sock"))?;
            let request = Request::Store(StoreRequest {
                input: input::store_input(args.input),
            });
            let response = client.request(request)?;
            render::render(response).map_err(CliError::Render)
        }
        (None, Some(Command::Daemon(args))) => {
            let daemon = Daemon::build(DaemonConfig {
                database_path: args.database,
                socket_path: args.socket,
            })?;
            daemon.run()?;
            Ok(())
        }
        (None, Some(Command::Status(args))) => {
            let client = IpcClient::connect(&args.socket)?;
            let response = client.request(Request::Status(
                crate::infrastructure::ipc::protocol::StatusRequest,
            ))?;
            render::render(response).map_err(CliError::Render)
        }
        (Some(_), Some(_)) => unreachable!("clap does not allow a question and subcommand together"),
        (None, None) => Err(CliError::Usage(
            "a question or command is required; try `recall --help`".to_owned(),
        )),
    }
}

/// Errors crossing the CLI runtime boundary.
#[derive(Debug)]
pub enum CliError {
    /// IPC connection or request failure.
    Ipc(IpcClientError),
    /// Daemon composition failure.
    Composition(crate::runtime::composition::CompositionError),
    /// Daemon lifecycle/transport failure.
    Daemon(crate::infrastructure::ipc::server::IpcServerError),
    /// Response rendering failure.
    Render(render::RenderError),
    /// Invalid CLI invocation.
    Usage(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ipc(error) => write!(formatter, "{error}"),
            Self::Composition(error) => write!(formatter, "{error}"),
            Self::Daemon(error) => write!(formatter, "{error}"),
            Self::Render(error) => write!(formatter, "{error}"),
            Self::Usage(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<IpcClientError> for CliError {
    fn from(error: IpcClientError) -> Self { Self::Ipc(error) }
}

impl From<crate::runtime::composition::CompositionError> for CliError {
    fn from(error: crate::runtime::composition::CompositionError) -> Self { Self::Composition(error) }
}

impl From<crate::infrastructure::ipc::server::IpcServerError> for CliError {
    fn from(error: crate::infrastructure::ipc::server::IpcServerError) -> Self { Self::Daemon(error) }
}
