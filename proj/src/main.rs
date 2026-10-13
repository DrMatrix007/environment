use anyhow::{Context, Result, ensure};
use clap::Parser;
use dialoguer::FuzzySelect;
use dialoguer::console::{Style, style};
use dialoguer::theme::ColorfulTheme;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

#[derive(Parser)]
struct Cli {
    #[arg(default_value = "")]
    name: String,
    #[arg(long)]
    attached: bool,
    #[arg(long)]
    focus: bool,
}

#[derive(Deserialize)]
struct Config {
    root: String,
    herdr_session: String,
    #[serde(default)]
    paths: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct SessionList {
    sessions: Vec<SessionInfo>,
}

#[derive(Deserialize)]
struct SessionInfo {
    name: String,
    running: bool,
}

#[derive(Deserialize)]
struct WorkspaceListResponse {
    result: WorkspaceListResult,
}

#[derive(Deserialize)]
struct WorkspaceListResult {
    workspaces: Vec<WorkspaceInfo>,
}

#[derive(Deserialize)]
struct WorkspaceInfo {
    workspace_id: String,
    label: String,
}

fn expand_tilde(value: &str, home: &Path) -> PathBuf {
    match value.strip_prefix('~') {
        Some(rest) => home.join(rest.trim_start_matches(['/', '\\'])),
        None => PathBuf::from(value),
    }
}

fn list_candidates(root: &Path, paths: &BTreeMap<String, String>, home: &Path) -> Result<Vec<(String, PathBuf)>> {
    let names: Vec<String> = std::fs::read_dir(root)
        .with_context(|| format!("reading {}", root.display()))?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();

    let mut candidates: Vec<(String, PathBuf)> =
        names.into_iter().map(|name| (name.clone(), root.join(&name))).collect();
    candidates.extend(paths.iter().map(|(name, path)| (name.clone(), expand_tilde(path, home))));
    candidates.sort_by_key(|(name, _)| name.to_lowercase());
    Ok(candidates)
}

fn resolve(candidates: &[(String, PathBuf)], query: &str) -> Result<Option<(String, PathBuf)>> {
    if let Some(found) = candidates.iter().find(|(name, _)| name == query) {
        return Ok(Some(found.clone()));
    }

    let needle = query.to_lowercase();
    let mut matches = candidates.iter().filter(|(name, _)| name.to_lowercase().contains(&needle));
    if let (Some(only), None) = (matches.next(), matches.next()) {
        return Ok(Some(only.clone()));
    }

    if !std::io::stdin().is_terminal() {
        anyhow::bail!("no match for {query:?} and stdin isn't a terminal to prompt on");
    }

    let names: Vec<String> = candidates.iter().map(|(name, _)| name.clone()).collect();
    let theme = ColorfulTheme {
        prompt_prefix: style("◆".into()).for_stderr().magenta(),
        active_item_prefix: style("❯".into()).for_stderr().magenta().bold(),
        active_item_style: Style::new().for_stderr().cyan().bold(),
        fuzzy_match_highlight_style: Style::new().for_stderr().yellow().bold(),
        ..ColorfulTheme::default()
    };
    let choice = FuzzySelect::with_theme(&theme).items(&names).with_initial_text(query).interact_opt()?;
    Ok(choice.map(|i| candidates[i].clone()))
}

fn session_is_running(stdout: &[u8], name: &str) -> Result<bool> {
    let list: SessionList = serde_json::from_slice(stdout)?;
    Ok(list.sessions.iter().any(|s| s.name == name && s.running))
}

fn session_running(session: &str) -> Result<bool> {
    let output = Command::new("herdr").args(["session", "list", "--json"]).output()?;
    ensure!(output.status.success(), "herdr session list exited with {}", output.status);
    session_is_running(&output.stdout, session)
        .with_context(|| String::from_utf8_lossy(&output.stderr).into_owned())
}

fn ensure_session_running(session: &str) -> Result<()> {
    if session_running(session)? {
        return Ok(());
    }
    Command::new("herdr")
        .args(["--session", session, "server"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("starting herdr session '{session}'"))?;

    for _ in 0..50 {
        if session_running(session)? {
            return Ok(());
        }
        sleep(Duration::from_millis(100));
    }
    anyhow::bail!("herdr session '{session}' did not start in time")
}

fn find_workspace_id(stdout: &[u8], label: &str) -> Result<Option<String>> {
    let response: WorkspaceListResponse = serde_json::from_slice(stdout)?;
    Ok(response.result.workspaces.into_iter().find(|w| w.label == label).map(|w| w.workspace_id))
}

fn find_workspace(session: &str, label: &str) -> Result<Option<String>> {
    let output = Command::new("herdr").args(["--session", session, "workspace", "list"]).output()?;
    ensure!(output.status.success(), "herdr workspace list exited with {}", output.status);
    find_workspace_id(&output.stdout, label)
        .with_context(|| String::from_utf8_lossy(&output.stderr).into_owned())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let home = std::env::home_dir().context("no home directory")?;
    let config_path = home.join("Projects").join("environment").join("projects.toml");
    let config: Config = toml::from_str(
        &std::fs::read_to_string(&config_path).with_context(|| format!("reading {}", config_path.display()))?,
    )
    .with_context(|| format!("parsing {}", config_path.display()))?;

    let root = expand_tilde(&config.root, &home);
    let candidates = list_candidates(&root, &config.paths, &home)?;

    let Some((name, dir)) = resolve(&candidates, &cli.name)? else {
        return Ok(());
    };

    let session = match std::env::var("HERDR_SESSION") {
        Ok(value) if !value.is_empty() => value,
        _ => config.herdr_session,
    };

    ensure_session_running(&session)?;
    let existing = find_workspace(&session, &name)?;

    if cli.attached || cli.focus {
        match existing {
            Some(id) => {
                let status = Command::new("herdr")
                    .args(["--session", &session, "workspace", "focus", &id])
                    .status()?;
                ensure!(status.success(), "herdr workspace focus exited with {status}");
            }
            None => {
                let status = Command::new("herdr")
                    .args(["--session", &session, "workspace", "create", "--cwd"])
                    .arg(&dir)
                    .args(["--label", &name, "--focus"])
                    .status()?;
                ensure!(status.success(), "herdr workspace create exited with {status}");
            }
        }
        if cli.attached {
            let status = Command::new("herdr").args(["--session", &session]).status()?;
            ensure!(status.success(), "herdr exited with {status}");
        }
        return Ok(());
    }

    if existing.is_some() {
        println!("herdr workspace '{name}' already running");
        return Ok(());
    }

    let status = Command::new("herdr")
        .args(["--session", &session, "workspace", "create", "--cwd"])
        .arg(&dir)
        .args(["--label", &name, "--no-focus"])
        .status()?;
    ensure!(status.success(), "herdr workspace create exited with {status}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_names(names: &[&str], query: &str) -> Result<Option<String>> {
        let candidates: Vec<(String, PathBuf)> =
            names.iter().map(|n| (n.to_string(), PathBuf::from(n))).collect();
        Ok(resolve(&candidates, query)?.map(|(name, _)| name))
    }

    #[test]
    fn resolve_exact_match_wins_over_substring() {
        assert_eq!(resolve_names(&["foo", "foobar"], "foo").unwrap(), Some("foo".into()));
    }

    #[test]
    fn resolve_unique_substring_match_wins() {
        assert_eq!(resolve_names(&["environment", "mux", "proj"], "mu").unwrap(), Some("mux".into()));
    }

    #[test]
    fn resolve_substring_match_is_case_insensitive() {
        assert_eq!(resolve_names(&["Environment", "mux"], "ENV").unwrap(), Some("Environment".into()));
    }

    #[test]
    fn resolve_ambiguous_match_errors_without_tty() {
        assert!(resolve_names(&["foobar", "foobaz"], "foo").is_err());
    }

    #[test]
    fn resolve_empty_query_errors_without_tty_when_ambiguous() {
        assert!(resolve_names(&["foobar", "foobaz"], "").is_err());
    }

    #[test]
    fn resolve_empty_query_resolves_single_candidate() {
        assert_eq!(resolve_names(&["foobar"], "").unwrap(), Some("foobar".into()));
    }

    #[test]
    fn resolve_no_match_errors_without_tty() {
        assert!(resolve_names(&["foo", "bar"], "zzz").is_err());
    }

    #[test]
    fn expand_tilde_expands_against_injected_home() {
        let home = PathBuf::from("/fake/home");
        assert_eq!(expand_tilde("~/Projects", &home), home.join("Projects"));
        assert_eq!(expand_tilde("~/.claude", &home), home.join(".claude"));
    }

    #[test]
    fn list_candidates_merges_root_subdirs_and_paths() {
        let root = std::env::temp_dir().join(format!("proj-test-merge-{}", std::process::id()));
        std::fs::create_dir_all(root.join("subdir1")).unwrap();
        std::fs::create_dir_all(root.join("subdir2")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".hidden-file"), "").unwrap();

        let mut paths = BTreeMap::new();
        paths.insert("pathsKey".to_string(), "/elsewhere".to_string());

        let home = PathBuf::from("/fake/home");
        let result = list_candidates(&root, &paths, &home).unwrap();
        let mut names: Vec<String> = result.into_iter().map(|(name, _)| name).collect();
        names.sort();

        std::fs::remove_dir_all(&root).unwrap();

        assert_eq!(names, vec!["pathsKey".to_string(), "subdir1".to_string(), "subdir2".to_string()]);
    }

    #[test]
    fn list_candidates_sorts_case_insensitively() {
        let root = std::env::temp_dir().join(format!("proj-test-sort-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Banana")).unwrap();
        std::fs::create_dir_all(root.join("apple")).unwrap();

        let mut paths = BTreeMap::new();
        paths.insert("Cherry".to_string(), "/elsewhere".to_string());

        let home = PathBuf::from("/fake/home");
        let result = list_candidates(&root, &paths, &home).unwrap();
        let names: Vec<String> = result.into_iter().map(|(name, _)| name).collect();

        std::fs::remove_dir_all(&root).unwrap();

        assert_eq!(names, vec!["apple".to_string(), "Banana".to_string(), "Cherry".to_string()]);
    }

    #[test]
    fn session_is_running_true_when_running() {
        let stdout = br#"{"sessions":[{"name":"default","running":false},{"name":"projects","running":true}]}"#;
        assert!(session_is_running(stdout, "projects").unwrap());
    }

    #[test]
    fn session_is_running_false_when_stopped() {
        let stdout = br#"{"sessions":[{"name":"projects","running":false}]}"#;
        assert!(!session_is_running(stdout, "projects").unwrap());
    }

    #[test]
    fn session_is_running_false_when_absent() {
        let stdout = br#"{"sessions":[{"name":"default","running":false}]}"#;
        assert!(!session_is_running(stdout, "projects").unwrap());
    }

    #[test]
    fn find_workspace_id_returns_id_on_label_match() {
        let stdout = br#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"workspace_id":"w1","label":"environment"},{"workspace_id":"w2","label":"other"}]}}"#;
        assert_eq!(find_workspace_id(stdout, "other").unwrap(), Some("w2".to_string()));
    }

    #[test]
    fn find_workspace_id_none_when_no_match() {
        let stdout = br#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"workspace_id":"w1","label":"environment"}]}}"#;
        assert_eq!(find_workspace_id(stdout, "missing").unwrap(), None);
    }
}
