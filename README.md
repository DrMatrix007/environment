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
cargo run -- distribute-configurations     # copies configurations/tuios/config.toml -> %LOCALAPPDATA%\tuios\config.toml
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

## Projects + agents (tuios)

`proj <name>` (the `proj` crate in this repo) fuzzy-matches a directory under the `root` configured
in [`projects.toml`](projects.toml) (default `~/Projects`), or a hardcoded entry in its `[paths]`
table (currently just `.claude`). By default it runs `tuios new <name> --cwd <dir> --detach` to
create the named tuios session headless if one doesn't exist yet, and does nothing if it's already
running. With `--attached`, it instead runs `tuios attach <name> -c` in that directory, attaching
in the foreground and creating the session if it doesn't exist yet.

A project directory with a `.tuios.tape` file (see [`.tuios.tape`](.tuios.tape) for this repo) gets
offered that layout the first time tuios sees a shell in it; review and trust it once
(`Ctrl+B T t` inside tuios) and it builds automatically after that.

[`configurations/tuios/config.toml`](configurations/tuios/config.toml) is tuios's own config, tracked
in this repo and distributed by `cargo run -- full` / `cargo run -- distribute-configurations` to
`%LOCALAPPDATA%\tuios\config.toml`.

[`configurations/tuios/tools.tape`](configurations/tuios/tools.tape) is distributed the same way, to
`%LOCALAPPDATA%\tuios\tools.tape`. It's a reusable tape you can run anytime with `tuios tape exec tools`
to add a lazygit + terminal pane pair to whatever window you're in — distinct from `.tuios.tape`, which
only auto-builds when a shell enters this project's directory.
