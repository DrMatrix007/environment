mod config;
mod configs;
mod generate;
mod install;

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
    /// Add a line loading the generated script(s) to the shell profile(s): ~/.bashrc, $PROFILE.
    Install {
        /// Directory containing env.sh / env.ps1.
        #[arg(short, long, default_value = "dist")]
        out_dir: PathBuf,

        /// Only install for this shell. Repeatable; defaults to all shells.
        #[arg(short, long, value_enum)]
        shell: Vec<ShellKind>,

        /// Profile file to edit instead of the default (requires a single --shell).
        #[arg(long, requires = "shell")]
        profile: Option<PathBuf>,
    },
    /// Parse and validate a config file without generating anything.
    Check {
        #[arg(short, long, default_value = "environment.toml")]
        config: PathBuf,
    },
    /// Copy static config files (psmux.conf, ...) to their target locations (e.g. ~/.psmux.conf).
    DistributeConfigurations,
    /// Generate, install, and distribute-configurations in one shot (full setup).
    Full {
        /// Path to the TOML config.
        #[arg(short, long, default_value = "environment.toml")]
        config: PathBuf,

        /// Directory to write env.sh / env.ps1 into.
        #[arg(short, long, default_value = "dist")]
        out_dir: PathBuf,
    },
}

fn file_name(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => "env.sh",
        ShellKind::Powershell => "env.ps1",
    }
}

fn write_script(config: &Config, shell: ShellKind, out_dir: &std::path::Path) -> Result<PathBuf> {
    let path = out_dir.join(file_name(shell));
    let mut script = generate::generate(config, shell);
    if shell == ShellKind::Powershell {
        script.insert(0, '\u{feff}');
    }
    std::fs::write(&path, script).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(path)
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
                write_script(&config, shell, &out_dir)?;
            }
        }
        Command::Install { out_dir, shell, profile } => {
            let shells = if shell.is_empty() { vec![ShellKind::Bash, ShellKind::Powershell] } else { shell };
            anyhow::ensure!(profile.is_none() || shells.len() == 1, "--profile needs exactly one --shell");
            for shell in shells {
                install::install(shell, &out_dir.join(file_name(shell)), profile.clone())?;
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
        Command::DistributeConfigurations => {
            configs::distribute()?;
        }
        Command::Full { config, out_dir } => {
            // Exclude our own package: it's already built (we're running as it), and on
            // Windows a running exe can't be overwritten by the build that would rebuild it.
            let status = std::process::Command::new("cargo")
                .args(["build", "--release", "--workspace", "--exclude", "environment"])
                .status()
                .context("running cargo build --release --workspace")?;
            anyhow::ensure!(status.success(), "cargo build --release --workspace failed");

            let config = Config::load(&config)?;
            std::fs::create_dir_all(&out_dir)
                .with_context(|| format!("creating {}", out_dir.display()))?;
            for shell in [ShellKind::Bash, ShellKind::Powershell] {
                let path = write_script(&config, shell, &out_dir)?;
                install::install(shell, &path, None)?;
            }
            configs::distribute()?;
        }
    }
    Ok(())
}
