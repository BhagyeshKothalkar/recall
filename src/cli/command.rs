//! User-facing command model.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Recall command-line interface.
#[derive(Debug, Parser)]
#[command(name = "recall", version, about = "A local personal memory system")]
pub struct Cli {
    /// Primary question form: `recall "what did I decide?"`.
    #[arg(value_name = "QUESTION")]
    pub question: Option<String>,

    /// Retrieve memories without invoking the AI backend.
    #[arg(long)]
    pub no_ai: bool,

    /// Unix-domain socket path used by client commands and the daemon.
    #[arg(long, global = true)]
    pub socket: Option<PathBuf>,

    /// Explicit operational command, when one is required.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Explicit Recall commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Store text or the contents of an existing file.
    Store(StoreArgs),
    /// Start the Recall daemon.
    Daemon(DaemonArgs),
    /// Query daemon status.
    Status(StatusArgs),
    /// Start the Recall daemon in the background.
    Start(DaemonArgs),
    /// Stop the running Recall daemon.
    Stop(StopArgs),
}

/// Arguments for `recall store`.
#[derive(Debug, Args)]
pub struct StoreArgs {
    /// Text to store, or an existing file path.
    pub input: String,
}

/// Arguments for `recall daemon`.
#[derive(Debug, Args)]
pub struct DaemonArgs {
    /// SQLite database path.
    #[arg(long)]
    pub database: Option<PathBuf>,
}

/// Arguments for `recall status`.
#[derive(Debug, Args)]
pub struct StatusArgs {}

/// Arguments for `recall stop`.
#[derive(Debug, Args)]
pub struct StopArgs {}
