use anyhow::{Context, Result};
use dialoguer::console::{Style, style};
use dialoguer::{Confirm, FuzzySelect, theme::ColorfulTheme};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use xshell::{Shell, cmd};

const TOOLS_SESSION: &str = "tools";
const AI_SESSION: &str = "ai";

/// Directories outside ~/Projects, as (name, path relative to home).
const HARDCODED: &[(&str, &str)] = &[(".claude", ".claude")];

pub fn window_name(name: &str) -> String {
    name.replace('.', "_")
}

pub fn list_projects(home: &Path) -> Result<Vec<(String, PathBuf)>> {
    let root = home.join("Projects");
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .with_context(|| format!("reading {}", root.display()))?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.extend(HARDCODED.iter().map(|(name, _)| name.to_string()));
    names.sort_by_key(|p| p.to_lowercase());
    Ok(names
        .into_iter()
        .map(|name| {
            let dir = match HARDCODED.iter().find(|(n, _)| *n == name) {
                Some((_, rel)) => home.join(rel),
                None => root.join(&name),
            };
            (name, dir)
        })
        .collect())
}

pub fn pick(home: &Path, query: Option<&str>) -> Result<Option<(String, PathBuf)>> {
    let projects = list_projects(home)?;
    let names = projects.iter().map(|(name, _)| name.clone()).collect();
    let Some(name) = fuzzy_pick(names, query, "project")? else {
        return Ok(None);
    };
    Ok(projects.into_iter().find(|(n, _)| *n == name))
}

pub fn create(home: &Path, name: &str) -> Result<(String, PathBuf)> {
    let dir = home.join("Projects").join(name);
    std::fs::create_dir(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok((name.to_string(), dir))
}

/// Resolves `query` against `items`: an exact match or a single unique substring match is
/// returned without prompting; otherwise shows a fuzzy-select prompt (bailing if stdin isn't a
/// terminal to prompt on). Returns None if the user cancels the prompt.
fn fuzzy_pick(mut items: Vec<String>, query: Option<&str>, prompt: &str) -> Result<Option<String>> {
    let query = query.unwrap_or_default();
    if items.iter().any(|i| i == query) {
        return Ok(Some(query.into()));
    }
    let needle = query.to_lowercase();
    let mut matches = items.iter().filter(|i| i.to_lowercase().contains(&needle));
    if let (false, Some(only), None) = (query.is_empty(), matches.next(), matches.next()) {
        return Ok(Some(only.clone()));
    }

    if !std::io::stdin().is_terminal() {
        anyhow::bail!("no match for {query:?} and stdin isn't a terminal to prompt on");
    }

    let theme = ColorfulTheme {
        prompt_prefix: style("◆".into()).for_stderr().magenta(),
        active_item_prefix: style("❯".into()).for_stderr().magenta().bold(),
        active_item_style: Style::new().for_stderr().cyan().bold(),
        fuzzy_match_highlight_style: Style::new().for_stderr().yellow().bold(),
        ..ColorfulTheme::default()
    };
    let choice = FuzzySelect::with_theme(&theme)
        .with_prompt(prompt)
        .with_initial_text(query)
        .items(&items)
        .default(0)
        .max_length(12)
        .interact_opt()?;
    Ok(choice.map(|i| items.swap_remove(i)))
}

pub fn confirm(prompt: &str) -> Result<bool> {
    Ok(Confirm::new().with_prompt(prompt).default(false).interact()?)
}

pub struct Wt {
    sh: Shell,
    name: String,
    dir: PathBuf,
}

impl Wt {
    pub fn new(sh: Shell, name: String, dir: PathBuf) -> Self {
        Self { sh, dir, name: window_name(&name) }
    }

    /// Ensures the project has a window in both the shared `tools` and `ai` sessions, and ends
    /// focused on the tools window.
    pub fn open(&self) -> Result<()> {
        self.ai()?;
        self.tools()?;
        Ok(())
    }

    /// Kills the project's window in both the shared `tools` and `ai` sessions, the inverse of
    /// `open`.
    pub fn close(&self) -> Result<()> {
        self.kill_window(AI_SESSION)?;
        self.kill_window(TOOLS_SESSION)?;
        Ok(())
    }

    fn kill_window(&self, session: &str) -> Result<()> {
        let Self { sh, name, .. } = self;
        if self.window_exists(session, name)? {
            cmd!(sh, "psmux kill-window -t {session}:{name}").run()?;
        }
        Ok(())
    }

    fn window_exists(&self, session: &str, window: &str) -> Result<bool> {
        let Self { sh, .. } = self;
        let fmt = "#{window_name}";
        let Ok(windows) = cmd!(sh, "psmux list-windows -t {session} -F {fmt}").quiet().read() else {
            return Ok(false);
        };
        Ok(windows.lines().any(|w| w == window))
    }

    pub fn tools(&self) -> Result<()> {
        let Self { sh, name, dir } = self;
        let window = format!("{TOOLS_SESSION}:{name}");
        let had_window = self.window_exists(TOOLS_SESSION, name)?;
        cmd!(sh, "psmux new-session -A -d -s {TOOLS_SESSION} -n {name} -c {dir}").run()?;
        if !had_window {
            if !self.window_exists(TOOLS_SESSION, name)? {
                cmd!(sh, "psmux new-window -d -t {TOOLS_SESSION} -n {name} -c {dir}").run()?;
            }
            self.setup_tools(&window)?;
        }
        cmd!(sh, "psmux select-window -t {window}").run()?;
        cmd!(sh, "psmux switch-client -t {TOOLS_SESSION}").run()?;
        Ok(())
    }

    fn setup_tools(&self, window: &str) -> Result<()> {
        let Self { sh, dir, .. } = self;
        cmd!(sh, "psmux send-keys -t {window} lazygit Enter").run()?;
        cmd!(sh, "psmux split-window -h -t {window} -c {dir}").run()?;
        Ok(())
    }

    fn ai_impl(&self, add_pane_if_open: bool) -> Result<()> {
        let Self { sh, name, dir } = self;
        let window = format!("{AI_SESSION}:{name}");
        let had_window = self.window_exists(AI_SESSION, name)?;
        cmd!(
            sh,
            "psmux new-session -A -d -s {AI_SESSION} -n {name} -c {dir} -- pwsh -NoExit -Command claude"
        )
        .run()?;
        if had_window {
            if add_pane_if_open {
                cmd!(sh, "psmux split-window -d -t {window} -c {dir} -- pwsh -NoExit -Command claude").run()?;
            }
        } else if !self.window_exists(AI_SESSION, name)? {
            cmd!(
                sh,
                "psmux new-window -d -t {AI_SESSION} -n {name} -c {dir} -- pwsh -NoExit -Command claude"
            )
            .run()?;
        }
        cmd!(sh, "psmux select-window -t {window}").run()?;
        cmd!(sh, "psmux switch-client -t {AI_SESSION}").run()?;
        Ok(())
    }

    /// claude pane in the project's window in the shared `ai` session (adds a pane if it's
    /// already open).
    pub fn ai(&self) -> Result<()> {
        self.ai_impl(true)
    }

    /// Selects/switches to the project's window in the shared `ai` session without adding a
    /// new claude pane if it's already open.
    pub fn select_ai(&self) -> Result<()> {
        self.ai_impl(false)
    }
}
