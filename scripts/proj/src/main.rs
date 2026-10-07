use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::console::{Style, style};
use dialoguer::{FuzzySelect, theme::ColorfulTheme};
use std::path::{Path, PathBuf};
use xshell::{Shell, cmd};

const AI_SESSION: &str = "ai";
const TOOLS_SESSION: &str = "tools";

#[derive(Parser)]
#[command(about = "Open a ~/Projects workspace in psmux")]
enum Cli {
    /// lazygit | shell window in psmux session "tools" (focuses the project's existing window, or creates it).
    Tools { query: Option<String> },
    /// claude pane in psmux session "ai" (adds a window per project; adds a pane to that project's window if it's already open).
    Ai { query: Option<String> },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (Cli::Tools { query } | Cli::Ai { query }) = &cli;
    let home = std::env::home_dir().context("no home directory")?;
    let Some((name, dir)) = pick(&home, query.as_deref())? else { return Ok(()) };
    let wt = Wt { sh: Shell::new()?, dir, name };

    match cli {
        Cli::Tools { .. } => wt.tools(),
        Cli::Ai { .. } => wt.ai(),
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

struct Wt {
    sh: Shell,
    name: String,
    dir: PathBuf,
}

impl Wt {
    fn tools(&self) -> Result<()> {
        let Self { sh, name, dir } = self;
        let window = format!("{TOOLS_SESSION}:{name}");
        let exists = cmd!(sh, "psmux has-session -t {TOOLS_SESSION}").quiet().run().is_ok();
        if !exists {
            cmd!(sh, "psmux new-session -d -s {TOOLS_SESSION} -n {name} -c {dir}").run()?;
            self.setup_tools(&window)?;
        } else {
            let fmt = "#{window_name}";
            let windows = cmd!(sh, "psmux list-windows -t {TOOLS_SESSION} -F {fmt}").read()?;
            if !windows.lines().any(|line| line == name) {
                cmd!(sh, "psmux new-window -d -t {TOOLS_SESSION} -n {name} -c {dir}").run()?;
                self.setup_tools(&window)?;
            }
        }
        cmd!(sh, "psmux select-window -t {window}").run()?;
        Ok(())
    }

    fn setup_tools(&self, window: &str) -> Result<()> {
        let Self { sh, dir, .. } = self;
        cmd!(sh, "psmux send-keys -t {window} lazygit Enter").run()?;
        cmd!(sh, "psmux split-window -h -t {window} -c {dir}").run()?;
        Ok(())
    }

    fn ai(&self) -> Result<()> {
        let Self { sh, name, dir } = self;
        let exists = cmd!(sh, "psmux has-session -t {AI_SESSION}").quiet().run().is_ok();
        if !exists {
            cmd!(
                sh,
                "psmux new-session -d -s {AI_SESSION} -n {name} -c {dir} -- pwsh -NoExit -Command claude"
            )
            .run()?;
            return Ok(());
        }

        let fmt = "#{window_name}";
        let windows = cmd!(sh, "psmux list-windows -t {AI_SESSION} -F {fmt}").read()?;
        if windows.lines().any(|line| line == name) {
            cmd!(
                sh,
                "psmux split-window -d -t {AI_SESSION}:{name} -c {dir} -- pwsh -NoExit -Command claude"
            )
            .run()?;
        } else {
            cmd!(
                sh,
                "psmux new-window -d -t {AI_SESSION} -n {name} -c {dir} -- pwsh -NoExit -Command claude"
            )
            .run()?;
        }

        cmd!(sh, "psmux select-window -t {AI_SESSION}:{name}").run()?;
        Ok(())
    }
}
