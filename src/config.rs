use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub env: IndexMap<String, PerShell>,

    #[serde(default)]
    pub path: Vec<PerShell>,

    #[serde(default)]
    pub aliases: IndexMap<String, PerShell>,

    #[serde(default)]
    pub functions: IndexMap<String, PerShell>,
}

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
