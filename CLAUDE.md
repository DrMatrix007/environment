# environment

After any change in this repo, run `cargo run -- full` so the generated config and shell profile install
match the repo.

## tuios

- `proj <name>` is the Rust binary built from the `proj` crate in this repo; it fuzzy-matches a directory under the `root` configured in `projects.toml` (default `~/Projects`), or a hardcoded entry in `projects.toml`'s `[paths]` table (currently just `.claude`). By default it runs `tuios new <name> --cwd <dir> --detach` in that directory to create the named tuios session headless if one doesn't exist yet, and does nothing if it's already running. With `--attached`, it instead runs `tuios attach <name> -c` in that directory, attaching in the foreground and creating the session if it doesn't exist yet.
- tuios (https://tuios.dev) is a terminal multiplexer/window manager with persistent daemon-backed sessions and native coding-agent tracking; it replaced an earlier hand-rolled tmux+Rust-TUI setup (`psmux.conf` + `scripts/proj`) that used to live in this repo and has since been removed.
- A project directory can have a `.tuios.tape` file (see `.tuios.tape` at this repo's root for a live example: it opens an `agent` pane running `claude` and a `git` pane running `lazygit`) that tuios offers to build into a session layout the first time a shell enters that directory. It must be reviewed and trusted once (`Ctrl+B T t` inside tuios) before it runs.
- tuios's config is tracked at `configurations/tuios/config.toml` in this repo and is distributed to `%LOCALAPPDATA%\tuios\config.toml` (Windows) by `cargo run -- full` / `cargo run -- distribute-configurations`. `tuios set-config <path> <value>` edits the live file directly (e.g. `appearance.preferred_shell` is currently set to `pwsh`), and `tuios list-options` lists every option; changes made this way should be copied back into the repo file, since a `full` re-run overwrites the live file from the repo copy.
- `configurations/tuios/tools.tape` is distributed the same way, to `%LOCALAPPDATA%\tuios\tools.tape`; it's a reusable named tape (`tuios tape exec tools`) that adds a `lazygit` + plain shell pane pair to whatever window you're in, distinct from `.tuios.tape` which only auto-builds on entering this project's directory.
