use anyhow::{Context, Result};
use clap::Parser;
use project::{Wt, pick};
use std::path::Path;
use xshell::{Shell, cmd};

mod project;
mod tui;

#[derive(Parser)]
#[command(about = "Open a ~/Projects workspace in psmux")]
enum Cli {
    /// tools window (lazygit+shell) in the shared `tools` session and ai window (claude) in the
    /// shared `ai` session, both named after the project (ends focused on the tools window).
    Open { query: Option<String> },
    /// claude pane in the project's window in the shared `ai` session (adds a pane if it's already open).
    Ai {
        query: Option<String>,
        /// use the current psmux session's name as the query
        #[arg(long, conflicts_with = "query")]
        here: bool,
    },
    /// Full-screen TUI to browse, open/switch to, or create ~/Projects workspaces.
    Manage { query: Option<String> },
    /// mkdir ~/Projects/<name>, then open it like `open`.
    Create { name: String },
    /// Kill the project's windows in both the `tools` and `ai` sessions (asks for confirmation first).
    Close { query: Option<String> },
}

fn main() -> Result<()> {
    let home = std::env::home_dir().context("no home directory")?;
    match Cli::parse() {
        Cli::Manage { query } => tui::run(&home, query.as_deref()),
        cli => open_or_create_or_ai(&home, cli),
    }
}

fn open_or_create_or_ai(home: &Path, cli: Cli) -> Result<()> {
    let wt = match &cli {
        Cli::Create { name } => {
            let (name, dir) = project::create(home, name)?;
            Wt::new(Shell::new()?, name, dir)
        }
        Cli::Open { query } => {
            let sh = Shell::new()?;
            let Some((name, dir)) = pick(home, query.as_deref())? else {
                return Ok(());
            };
            Wt::new(sh, name, dir)
        }
        Cli::Ai { query, here } => {
            let sh = Shell::new()?;
            let query = if *here {
                let fmt = "#{window_name}";
                Some(cmd!(sh, "psmux display-message -p {fmt}").read()?)
            } else {
                query.clone()
            };
            let Some((name, dir)) = pick(home, query.as_deref())? else {
                return Ok(());
            };
            Wt::new(sh, name, dir)
        }
        Cli::Close { query } => {
            let sh = Shell::new()?;
            let Some((name, dir)) = pick(home, query.as_deref())? else {
                return Ok(());
            };
            if !project::confirm(&format!("Close project '{name}'? This kills its tools and ai windows."))? {
                return Ok(());
            }
            Wt::new(sh, name, dir)
        }
        Cli::Manage { .. } => unreachable!("handled in main"),
    };

    match cli {
        Cli::Open { .. } => wt.open(),
        Cli::Ai { .. } => wt.ai(),
        Cli::Create { .. } => wt.open(),
        Cli::Close { .. } => wt.close(),
        Cli::Manage { .. } => unreachable!("handled in main"),
    }
}
