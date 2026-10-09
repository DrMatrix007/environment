# environment

After any change in this repo, run `cargo run -- full` so the generated config, shell profile install, and
distributed files (e.g. `~/.psmux.conf`) match the repo. Editing the files in `configurations/` alone doesn't
change the live setup.

Claude Code itself may be running inside a psmux pane (`proj ai` opens it in one). Be careful with
psmux commands that affect the whole server/session (e.g. `kill-server`, `kill-session`, reloading
`~/.psmux.conf` while testing) — they can disrupt the very pane Claude Code is running in.
