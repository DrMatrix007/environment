use anyhow::{Context, Result, ensure};
use clap::Parser;
use dialoguer::FuzzySelect;
use dialoguer::console::{Style, style};
use dialoguer::theme::ColorfulTheme;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
struct Cli {
    #[arg(default_value = "")]
    name: String,
    #[arg(long)]
    attached: bool,
}

#[derive(Deserialize)]
struct Config {
    root: String,
    #[serde(default)]
    paths: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Session {
    name: String,
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

fn session_matches(exit_code: i32, stdout: &[u8], target: &str) -> Result<bool> {
    ensure!(exit_code == 0 || exit_code == 3, "tuios ls --json exited with {exit_code}");
    let sessions: Vec<Session> = serde_json::from_slice(stdout)?;
    Ok(sessions.iter().any(|s| s.name == target))
}

fn session_running(name: &str) -> Result<bool> {
    let output = Command::new("tuios").args(["ls", "--json"]).output()?;
    let exit_code = output.status.code().unwrap_or(-1);
    session_matches(exit_code, &output.stdout, name)
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

    if cli.attached {
        let status = Command::new("tuios").args(["attach", &name, "-c"]).current_dir(&dir).status()?;
        ensure!(status.success(), "tuios attach exited with {status}");
        return Ok(());
    }

    if session_running(&name)? {
        println!("tuios session '{name}' already running");
        return Ok(());
    }

    let status = Command::new("tuios")
        .args(["new", &name, "--cwd"])
        .arg(&dir)
        .arg("--detach")
        .status()?;
    ensure!(status.success(), "tuios new exited with {status}");
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
    fn session_matches_true_when_target_present() {
        let stdout = br#"[{"name":"foo"},{"name":"bar"}]"#;
        assert!(session_matches(0, stdout, "foo").unwrap());
    }

    #[test]
    fn session_matches_false_when_target_absent() {
        let stdout = br#"[{"name":"foo"},{"name":"bar"}]"#;
        assert!(!session_matches(0, stdout, "baz").unwrap());
    }

    #[test]
    fn session_matches_ignores_unknown_fields() {
        let stdout = br#"[{"name":"foo","pid":123,"extra":{"a":1}}]"#;
        assert!(session_matches(0, stdout, "foo").unwrap());
    }

    #[test]
    fn session_matches_treats_exit_code_3_as_success() {
        let stdout = br#"[{"name":"foo"},{"name":"bar"}]"#;
        assert!(session_matches(3, stdout, "foo").unwrap());
    }

    #[test]
    fn session_matches_errors_on_other_exit_codes() {
        assert!(session_matches(1, b"[]", "foo").is_err());
    }
}
