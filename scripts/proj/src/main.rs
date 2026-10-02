use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::console::{Style, style};
use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use std::path::{Path, PathBuf};
use xshell::{Shell, cmd};

#[derive(Parser)]
#[command(about = "Open a ~/Projects workspace in tmux")]
enum Cli {
    /// lazygit | shell, in session "tools".
    Tools {
        query: Option<String>,
        /// Recreate the project's window.
        #[arg(short, long)]
        force: bool,
    },
    /// Add a claude pane to the project's window in session "ai"
    /// (only switches to it when the session is new).
    Ai { query: Option<String> },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (Cli::Tools { query, .. } | Cli::Ai { query }) = &cli;
    let home = std::env::home_dir().context("no home directory")?;
    let Some((name, dir)) = pick(&home, query.as_deref())? else { return Ok(()) };
    let tmux = Tmux { sh: Shell::new()?, dir, name: name.replace('.', "_") };

    match cli {
        Cli::Tools { force, .. } => tmux.tools(force),
        Cli::Ai { .. } => tmux.ai(),
    }
}

/// Directories outside ~/Projects, as (name, path relative to home).
const HARDCODED: &[(&str, &str)] = &[(".claude", ".claude")];

fn pick(home: &Path, query: Option<&str>) -> Result<Option<(String, PathBuf)>> {
    let root = home.join("Projects");
    let mut projects: Vec<String> = std::fs::read_dir(&root)
        .with_context(|| format!("reading {}", root.display()))?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    projects.extend(HARDCODED.iter().map(|(name, _)| name.to_string()));
    projects.sort_by_key(|p| p.to_lowercase());
    let resolve = |name: String| {
        let dir = match HARDCODED.iter().find(|(n, _)| *n == name) {
            Some((_, rel)) => home.join(rel),
            None => root.join(&name),
        };
        (name, dir)
    };

    let query = query.unwrap_or_default();
    if projects.iter().any(|p| p == query) {
        return Ok(Some(resolve(query.into())));
    }
    let needle = query.to_lowercase();
    let mut matches = projects.iter().filter(|p| p.to_lowercase().contains(&needle));
    if let (false, Some(only), None) = (query.is_empty(), matches.next(), matches.next()) {
        return Ok(Some(resolve(only.clone())));
    }

    let theme = ColorfulTheme {
        prompt_prefix: style("◆".into()).for_stderr().magenta(),
        active_item_prefix: style("❯".into()).for_stderr().magenta().bold(),
        active_item_style: Style::new().for_stderr().cyan().bold(),
        fuzzy_match_highlight_style: Style::new().for_stderr().yellow().bold(),
        ..ColorfulTheme::default()
    };
    let choice = FuzzySelect::with_theme(&theme)
        .with_prompt("project")
        .with_initial_text(query)
        .items(&projects)
        .default(0)
        .max_length(12)
        .interact_opt()?;
    Ok(choice.map(|i| resolve(projects.swap_remove(i))))
}

struct Tmux {
    sh: Shell,
    name: String,
    dir: PathBuf,
}

impl Tmux {
    fn tools(&self, force: bool) -> Result<()> {
        let Self { sh, name, dir } = self;
        let window = format!("=tools:{name}");
        if !self.has_session("=tools") {
            cmd!(sh, "tmux new-session -d -s tools -n {name} -c {dir}").run()?;
            self.setup_tools(&window)?;
        } else if !self.has_window("=tools", name) {
            cmd!(sh, "tmux new-window -t =tools: -n {name} -c {dir}").run()?;
            self.setup_tools(&window)?;
        } else if force {
            let old = format!("=tools:old-{name}");
            cmd!(sh, "tmux rename-window -t {window} old-{name}").run()?;
            cmd!(sh, "tmux new-window -t =tools: -n {name} -c {dir}").run()?;
            cmd!(sh, "tmux kill-window -t {old}").run()?;
            self.setup_tools(&window)?;
        }
        self.enter("=tools", &window)
    }

    fn setup_tools(&self, window: &str) -> Result<()> {
        let Self { sh, dir, .. } = self;
        cmd!(sh, "tmux send-keys -t {window} lazygit Enter").run()?;
        cmd!(sh, "tmux split-window -h -t {window} -c {dir}").run()?;
        Ok(())
    }

    fn ai(&self) -> Result<()> {
        let Self { sh, name, dir } = self;
        let window = format!("=ai:{name}");
        if !self.has_session("=ai") {
            cmd!(sh, "tmux new-session -d -s ai -n {name} -c {dir} claude").run()?;
            return self.enter("=ai", &window);
        }
        if self.has_window("=ai", name) {
            cmd!(sh, "tmux split-window -h -t {window} -c {dir} claude").run()?;
            cmd!(sh, "tmux select-layout -t {window} tiled").run()?;
        } else {
            cmd!(sh, "tmux new-window -d -t =ai: -n {name} -c {dir} claude").run()?;
        }
        Ok(())
    }

    fn enter(&self, session: &str, window: &str) -> Result<()> {
        let sh = &self.sh;
        cmd!(sh, "tmux select-window -t {window}").run()?;
        if std::env::var_os("TMUX").is_some() {
            cmd!(sh, "tmux switch-client -t {session}").run()?;
        } else {
            cmd!(sh, "tmux attach -t {session}").run()?;
        }
        Ok(())
    }

    fn has_session(&self, session: &str) -> bool {
        let sh = &self.sh;
        cmd!(sh, "tmux has-session -t {session}").quiet().ignore_stderr().run().is_ok()
    }

    fn has_window(&self, session: &str, name: &str) -> bool {
        let (sh, format) = (&self.sh, "#{window_name}");
        cmd!(sh, "tmux list-windows -t {session} -F {format}")
            .quiet()
            .ignore_stderr()
            .read()
            .is_ok_and(|names| names.lines().any(|n| n == name))
    }
}
