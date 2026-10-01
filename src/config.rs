use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::Deserialize;
use std::path::Path;

/// Top-level configuration. Maps preserve file order so later entries can
/// reference earlier ones (e.g. an env var built from another).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Environment variables to export.
    #[serde(default)]
    pub env: IndexMap<String, PerShell>,

    /// Directories prepended to PATH. A leading `~` expands to the home directory.
    #[serde(default)]
    pub path: Vec<PerShell>,

    /// Command aliases. Extra arguments are forwarded to the command.
    #[serde(default)]
    pub aliases: IndexMap<String, PerShell>,

    /// Shell functions. Bodies are shell-specific, so usually given per shell.
    #[serde(default)]
    pub functions: IndexMap<String, PerShell>,
}

/// A value that is either shared by all shells or specified per shell.
/// When given per shell, omitting a shell skips the entry for that shell.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PerShell {
    All(String),
    Split {
        bash: Option<String>,
        powershell: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ShellKind {
    Bash,
    Powershell,
}

impl PerShell {
    pub fn for_shell(&self, shell: ShellKind) -> Option<&str> {
        match self {
            PerShell::All(v) => Some(v),
            PerShell::Split { bash, powershell } => match shell {
                ShellKind::Bash => bash.as_deref(),
                ShellKind::Powershell => powershell.as_deref(),
            },
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self> {
        let config: Config = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        for name in self.env.keys() {
            let mut chars = name.chars();
            let valid_start = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
            if !valid_start || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
                bail!("invalid environment variable name: {name:?}");
            }
        }
        for (kind, names) in [("alias", &self.aliases), ("function", &self.functions)] {
            for name in names.keys() {
                let valid = !name.is_empty()
                    && !name.starts_with('-')
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c));
                if !valid {
                    bail!("invalid {kind} name: {name:?}");
                }
            }
        }
        for name in self.aliases.keys() {
            if self.functions.contains_key(name) {
                bail!("{name:?} is defined as both an alias and a function");
            }
        }
        Ok(())
    }
}
