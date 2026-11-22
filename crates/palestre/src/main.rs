//! The `palestre` command-line tool.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "palestre", version, about = "The Palestre PBC virtual machine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the remote execution server (SPEC annex K). Required by the evaluation.
    Exec {
        /// Address to listen on, for example 127.0.0.1:9000.
        #[arg(long)]
        listen: String,
    },
    /// Run one agent locally, for testing.
    Run {
        /// The `.pbc` file to run.
        program: PathBuf,
    },
    /// Play a match between two agents (SPEC annex E).
    Match {
        /// The first agent's `.pbc` file.
        first: PathBuf,
        /// The second agent's `.pbc` file.
        second: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Exec { listen } => {
            eprintln!("palestre exec --listen {listen}: not implemented yet");
        }
        Command::Run { program } => {
            eprintln!("palestre run {}: not implemented yet", program.display());
        }
        Command::Match { first, second } => {
            eprintln!(
                "palestre match {} {}: not implemented yet",
                first.display(),
                second.display()
            );
        }
    }
    eprintln!("(spec {})", pbc::SPEC_VERSION);
    ExitCode::FAILURE
}
