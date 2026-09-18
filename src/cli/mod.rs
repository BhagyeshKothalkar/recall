//! CLI orchestration.

pub mod command;
pub mod input;
pub mod render;

use std::{
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::infrastructure::ipc::client::{IpcClient, IpcClientError};
use crate::infrastructure::ipc::protocol::{Request, StopRequest, StoreRequest};
use crate::runtime::daemon::{Daemon, DaemonConfig};
use crate::runtime::paths::RecallPaths;
use command::{Cli, Command};

/// Runs the parsed CLI command.
pub fn run(cli: Cli) -> Result<(), CliError> {
    let no_ai = cli.no_ai;
    let socket_override = cli.socket;
    match (cli.question, cli.command) {
        (Some(question), None) => {
            let client = IpcClient::connect(&client_socket(socket_override.as_deref())?)?;
            let response = client.request(Request::Ask(
                crate::infrastructure::ipc::protocol::AskRequest {
                    question,
                    use_ai: !no_ai,
                },
            ))?;
            render::render(response).map_err(CliError::Render)
        }
        (None, Some(Command::Store(args))) => {
            let client = IpcClient::connect(&client_socket(socket_override.as_deref())?)?;
            let request = Request::Store(StoreRequest {
                input: input::store_input(args.input),
            });
            let response = client.request(request)?;
            render::render(response).map_err(CliError::Render)
        }
        (None, Some(Command::Daemon(args))) => {
            let paths = RecallPaths::resolve(args.database.as_deref(), socket_override.as_deref())?;
            paths.create_parent_dirs()?;
            let daemon = Daemon::build(DaemonConfig {
                database_path: paths.database().to_path_buf(),
                socket_path: paths.socket().to_path_buf(),
            })?;
            daemon.run()?;
            Ok(())
        }
        (None, Some(Command::Start(args))) => {
            let paths = RecallPaths::resolve(args.database.as_deref(), socket_override.as_deref())?;
            paths.create_parent_dirs()?;
            let executable = std::env::current_exe().map_err(CliError::Paths)?;
            let mut process = ProcessCommand::new(executable);
            process
                .arg("daemon")
                .arg("--database")
                .arg(paths.database())
                .arg("--socket")
                .arg(paths.socket())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            process.spawn().map_err(CliError::Paths)?;
            wait_for_socket(paths.socket())?;
            println!("daemon: started");
            Ok(())
        }
        (None, Some(Command::Stop(_))) => {
            let client = IpcClient::connect(&client_socket(socket_override.as_deref())?)?;
            let response = client.request(Request::Stop(StopRequest))?;
            render::render(response).map_err(CliError::Render)
        }
        (None, Some(Command::Status(args))) => {
            let _ = args;
            let client = IpcClient::connect(&client_socket(socket_override.as_deref())?)?;
            let response = client.request(Request::Status(
                crate::infrastructure::ipc::protocol::StatusRequest,
            ))?;
            render::render(response).map_err(CliError::Render)
        }
        (Some(_), Some(_)) => {
            unreachable!("clap does not allow a question and subcommand together")
        }
        (None, None) => Err(CliError::Usage(
            "a question or command is required; try `recall --help`".to_owned(),
        )),
    }
}

fn client_socket(override_path: Option<&Path>) -> Result<PathBuf, CliError> {
    Ok(RecallPaths::resolve(None::<&Path>, override_path)?
        .socket()
        .to_path_buf())
}

fn wait_for_socket(path: &Path) -> Result<(), CliError> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match IpcClient::connect(path) {
            Ok(_) => return Ok(()),
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(CliError::Ipc(error)),
        }
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
    /// Runtime path resolution or directory setup failure.
    Paths(std::io::Error),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ipc(error) => write!(formatter, "{error}"),
            Self::Composition(error) => write!(formatter, "{error}"),
            Self::Daemon(error) => write!(formatter, "{error}"),
            Self::Render(error) => write!(formatter, "{error}"),
            Self::Usage(error) => write!(formatter, "{error}"),
            Self::Paths(error) => write!(formatter, "runtime path error: {error}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        Self::Paths(error)
    }
}

impl From<IpcClientError> for CliError {
    fn from(error: IpcClientError) -> Self {
        Self::Ipc(error)
    }
}

impl From<crate::runtime::composition::CompositionError> for CliError {
    fn from(error: crate::runtime::composition::CompositionError) -> Self {
        Self::Composition(error)
    }
}

impl From<crate::infrastructure::ipc::server::IpcServerError> for CliError {
    fn from(error: crate::infrastructure::ipc::server::IpcServerError) -> Self {
        Self::Daemon(error)
    }
}
