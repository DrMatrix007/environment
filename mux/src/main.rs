use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Daemon-backed terminal multiplexer")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new session.
    New {
        session: String,
    },
    /// Attach to a session, creating it first if missing.
    Attach {
        session: String,
    },
    /// Detach from a session.
    Detach {
        session: String,
    },
    /// List sessions.
    Ls,
    /// Kill a session.
    Kill {
        session: String,
    },
    /// Manage session layouts.
    Layout {
        #[command(subcommand)]
        command: LayoutCommand,
    },
    /// Run the mux daemon in the foreground.
    Daemon,
}

#[derive(Subcommand)]
enum LayoutCommand {
    /// Apply a layout file to a session.
    Apply {
        file: PathBuf,

        #[arg(long)]
        session: String,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::New { session } => {
            println!("not yet implemented: new {session}");
        }
        Command::Attach { session } => {
            println!("not yet implemented: attach {session}");
        }
        Command::Detach { session } => {
            println!("not yet implemented: detach {session}");
        }
        Command::Ls => {
            println!("not yet implemented: ls");
        }
        Command::Kill { session } => {
            println!("not yet implemented: kill {session}");
        }
        Command::Layout { command: LayoutCommand::Apply { file, session } } => {
            println!("not yet implemented: layout apply {} --session {session}", file.display());
        }
        Command::Daemon => {
            println!("not yet implemented: daemon");
        }
    }
    Ok(())
}
