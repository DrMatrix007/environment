mod config;
mod generate;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::{Config, ShellKind};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Generate bash and PowerShell environment scripts from a TOML config")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate shell scripts from a config file.
    Generate {
        /// Path to the TOML config.
        #[arg(short, long, default_value = "environment.toml")]
        config: PathBuf,

        /// Directory to write env.sh / env.ps1 into.
        #[arg(short, long, default_value = "dist")]
        out_dir: PathBuf,

        /// Only generate for this shell. Repeatable; defaults to all shells.
        #[arg(short, long, value_enum)]
        shell: Vec<ShellKind>,

        /// Print the script to stdout instead of writing files (requires a single --shell).
        #[arg(long, requires = "shell")]
        stdout: bool,
    },
    /// Parse and validate a config file without generating anything.
    Check {
        #[arg(short, long, default_value = "environment.toml")]
        config: PathBuf,
    },
}

fn file_name(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => "env.sh",
        ShellKind::Powershell => "env.ps1",
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Generate { config, out_dir, shell, stdout } => {
            let config = Config::load(&config)?;
            let shells = if shell.is_empty() { vec![ShellKind::Bash, ShellKind::Powershell] } else { shell };

            if stdout {
                anyhow::ensure!(shells.len() == 1, "--stdout needs exactly one --shell");
                print!("{}", generate::generate(&config, shells[0]));
                return Ok(());
            }

            std::fs::create_dir_all(&out_dir)
                .with_context(|| format!("creating {}", out_dir.display()))?;
            for shell in shells {
                let path = out_dir.join(file_name(shell));
                let mut script = generate::generate(&config, shell);
                if shell == ShellKind::Powershell {
                    // Windows PowerShell 5.1 reads BOM-less files as ANSI.
                    script.insert(0, '\u{feff}');
                }
                std::fs::write(&path, script).with_context(|| format!("writing {}", path.display()))?;
                println!("wrote {}", path.display());
            }
        }
        Command::Check { config } => {
            let c = Config::load(&config)?;
            println!(
                "{}: ok ({} env, {} path, {} aliases, {} functions)",
                config.display(),
                c.env.len(),
                c.path.len(),
                c.aliases.len(),
                c.functions.len()
            );
        }
    }
    Ok(())
}
