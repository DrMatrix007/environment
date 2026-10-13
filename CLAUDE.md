# environment

After any change in this repo, run `cargo run -- full` so the generated config and shell profile install
match the repo.

## proj

- `proj <name>` is the Rust binary built from the `proj` crate in this repo; it fuzzy-matches a directory under the `root` configured in `projects.toml` (default `~/Projects`), or a hardcoded entry in `projects.toml`'s `[paths]` table (currently just `.claude`). It targets the herdr session named by `$HERDR_SESSION` if that's set in the environment, otherwise `projects.toml`'s `herdr_session` key, starting that session's headless server first if it isn't running yet. By default it creates a herdr workspace labeled `<name>`, cwd'd into the matched directory, if one doesn't already exist, and does nothing if it's already running. With `--attached`, it creates or focuses that workspace and then attaches the terminal to the herdr session in the foreground. With `--focus`, it does the same create-or-focus step but skips the attach, for use from something already inside the herdr session (e.g. a `prefix+ctrl+p` popup keybinding runs `proj --focus` to switch the attached client to another project).
- herdr's config is tracked at `configurations/herdr/config.toml` in this repo and is distributed to `%APPDATA%\herdr\config.toml` (Windows) by `cargo run -- full` / `cargo run -- distribute-configurations`; changes made live should be copied back into the repo file, since a `full` re-run overwrites the live file from the repo copy.
