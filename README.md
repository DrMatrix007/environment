# environment

Generate bash and PowerShell environment scripts (env vars, PATH, aliases, functions) from one TOML config.

```sh
cargo run -- generate                      # environment.toml -> dist/env.sh, dist/env.ps1
cargo run -- generate -s bash --stdout     # print one script
cargo run -- check                         # validate the config only
```

Then load the output from your shell startup:

```sh
# ~/.bashrc
source /path/to/dist/env.sh
```

```powershell
# $PROFILE
. C:\path\to\dist\env.ps1
```

## Config

See [`environment.toml`](environment.toml). Every value is either a string used by all shells,
or a per-shell table `{ bash = "...", powershell = "..." }`; leaving a shell out skips the entry there.

| Section       | bash                         | PowerShell                                        |
|---------------|------------------------------|---------------------------------------------------|
| `[env]`       | `export NAME='value'`        | `$env:NAME = 'value'`                             |
| `path = [..]` | prepended, no duplicates     | prepended, no duplicates                          |
| `[aliases]`   | `alias name='cmd'`           | `function name { cmd @args }` (replaces built-in aliases) |
| `[functions]` | `name() { body }`            | `function name { body }`                          |

Values are written literally (single-quoted); a leading `~` in `path` entries expands to the home directory.
