//! Recall process entry point.

use std::process::ExitCode;

use clap::Parser;

use recall::cli::command::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match recall::cli::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("recall: {error}");
            ExitCode::FAILURE
        }
    }
}
