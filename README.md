# environment

Generate bash and PowerShell environment scripts (env vars, PATH, aliases, functions) from one TOML config.

```sh
cargo run -- generate                      # environment.toml -> dist/env.sh, dist/env.ps1
cargo run -- generate -s bash --stdout     # print one script
cargo run -- check                         # validate the config only
cargo run -- full                          # build --release --workspace + generate + install + distribute-configurations
```

Then load the output from your shell startup:

```sh
cargo run -- install                       # appends to ~/.bashrc and $PROFILE (skips if already there)
cargo run -- install -s bash --profile ~/.bash_profile
cargo run -- distribute-configurations     # copies configurations/psmux.conf -> ~/.psmux.conf
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

[`psmux.conf`](configurations/psmux.conf) configures psmux, the terminal multiplexer that `scripts/proj`'s
`ai` command drives. It sets `default-shell` to `pwsh` so `proj open` / `proj ai` (on PATH via the config above)
also work directly inside psmux panes.
`cargo run -- distribute-configurations` (or `full`) copies it to `~/.psmux.conf`.

## Scripts

`scripts/*` are small Rust CLIs in the same cargo workspace (clap for args, xshell for running commands,
dialoguer for prompts). `.cargo/config.toml` points the target dir at `dist/target`, and
`dist/target/release` is on PATH via `environment.toml`, so `cargo run -- full` (which builds the
whole workspace first) is all it takes to get `proj open|ai|manage [query]` working directly from the shell.
