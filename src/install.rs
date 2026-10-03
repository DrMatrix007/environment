use crate::config::ShellKind;
use crate::generate::{bash_quote, ps_quote};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Appends a line loading `script` to the shell's profile, unless that line is already there.
pub fn install(shell: ShellKind, script: &Path, profile: Option<PathBuf>) -> Result<()> {
    let script = std::path::absolute(script)
        .with_context(|| format!("resolving {}", script.display()))?;
    if !script.exists() {
        eprintln!("warning: {} does not exist yet, run `generate` first", script.display());
    }
    let profile = match profile {
        Some(p) => p,
        None => default_profile(shell)?,
    };

    let script = script.to_string_lossy();
    let line = match shell {
        // Git Bash / WSL-style shells accept forward slashes on Windows paths.
        ShellKind::Bash => format!("source {}", bash_quote(&script.replace('\\', "/"))),
        ShellKind::Powershell => format!(". {}", ps_quote(&script)),
    };

    let existing = match std::fs::read_to_string(&profile) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", profile.display())),
    };
    if existing.lines().any(|l| l.trim() == line) {
        println!("{}: already installed", profile.display());
        return Ok(());
    }

    if let Some(dir) = profile.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&line);
    text.push('\n');
    std::fs::write(&profile, text).with_context(|| format!("writing {}", profile.display()))?;
    println!("{}: added `{line}`", profile.display());
    Ok(())
}

fn default_profile(shell: ShellKind) -> Result<PathBuf> {
    match shell {
        ShellKind::Bash => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .context("neither HOME nor USERPROFILE is set")?;
            Ok(PathBuf::from(home).join(".bashrc"))
        }
        ShellKind::Powershell => {
            // Ask PowerShell itself, since $PROFILE depends on edition and OneDrive-redirected Documents.
            for exe in ["pwsh", "powershell"] {
                let Ok(out) = Command::new(exe)
                    .args(["-NoLogo", "-NoProfile", "-Command", "$PROFILE.CurrentUserCurrentHost"])
                    .output()
                else {
                    continue;
                };
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if out.status.success() && !path.is_empty() {
                    return Ok(PathBuf::from(path));
                }
            }
            bail!("could not run pwsh or powershell to find $PROFILE; pass --profile")
        }
    }
}
