# environment

Generate bash and PowerShell environment scripts (env vars, PATH, aliases, functions) from one TOML config.

```sh
cargo run -- generate                      # environment.toml -> dist/env.sh, dist/env.ps1
cargo run -- generate -s bash --stdout     # print one script
cargo run -- check                         # validate the config only
cargo run -- full                          # build --release --workspace (except environment) + generate + install + distribute-configurations
```

Then load the output from your shell startup:

```sh
cargo run -- install                       # appends to ~/.bashrc and $PROFILE (skips if already there)
cargo run -- install -s bash --profile ~/.bash_profile
cargo run -- distribute-configurations     # copies configurations/herdr/config.toml -> %APPDATA%\herdr\config.toml
```

which adds

```sh
# ~/.bashrc
source '/path/to/dist/env.sh'
```

```powershell
# $PROFILE
. 'C:\path\to\dist\env.ps1'
```

## Config

See [`environment.toml`](environment.toml). Every value is either a string used by all shells,
or a per-shell table `{ bash = "...", powershell = "..." }`; leaving a shell out skips the entry there.

| Section       | bash                         | PowerShell                                        |
|---------------|------------------------------|---------------------------------------------------|
| `[env]`       | `export NAME='value'`        | `$env:NAME = 'value'`                             |
| `path = [..]` | prepended, no duplicates     | prepended, no duplicates                          |
| `[aliases]`   | `alias name='cmd'`           | `function global:name { cmd @args }` (replaces built-in aliases) |
| `[functions]` | `name() { body }`            | `function global:name { body }`                   |

PowerShell functions also get the positional arguments as `$1`..`$9` and `2>/dev/null` becomes `2>$null`,
so one function body using plain commands, `"$1"`, `||`, `&&` and `2>/dev/null` works in both shells
(PowerShell 7+).

Values are written literally (single-quoted); a leading `~` in `path` entries expands to the home directory.

## Projects

`proj <name>` (the `proj` crate in this repo) fuzzy-matches a directory under the `root` configured
in [`projects.toml`](projects.toml) (default `~/Projects`), or a hardcoded entry in its `[paths]`
table (currently just `.claude`). It targets a herdr session (named by `$HERDR_SESSION` if set in the
environment, else `projects.toml`'s `herdr_session` key), starting that session's headless server
first if it isn't running yet. By default it creates a herdr workspace labeled `<name>`, cwd'd into
the matched directory, if one doesn't already exist, and does nothing if it's already running. With
`--attached`, it creates or focuses that workspace and then attaches the terminal to the herdr
session in the foreground. With `--focus`, it creates or focuses that workspace the same way but
skips attaching a terminal, switching whatever herdr client is already attached over to that
workspace instead.

[`configurations/herdr/config.toml`](configurations/herdr/config.toml) is herdr's own config, tracked
in this repo and distributed by `cargo run -- full` / `cargo run -- distribute-configurations` to
`%APPDATA%\herdr\config.toml`. It sets `keys.switch_tab = "alt+1..9"` so alt+1 through alt+9 jump
directly between tabs, and `keys.focus_agent = "ctrl+1..9"` so ctrl+1 through ctrl+9
jump directly to agent rows. It also binds `prefix+ctrl+g` to open `lazygit` in a popup, `alt+arrows`
to move between panes, `ctrl+alt+left`/`ctrl+alt+right` to cycle tabs, and
`ctrl+shift+alt+left`/`ctrl+shift+alt+right` to cycle workspaces, and `prefix+ctrl+p` to open a
popup running `proj --focus` to switch to another project.
