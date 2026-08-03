# Config

Version 0.3 introduces `devbox.toml` at the workspace root.

## Example

```toml
[workspace]
name = "Planning"

[environment]
DOTNET_ENVIRONMENT = "Development"

[services.api]
command = "dotnet"
args = ["run", "--project", "src/Api"]
cwd = "src"
environment = { LOG_LEVEL = "info" }

[services.redis]
command = "redis-server"

[tools.git]
default_version = "2.45.0"
executable = "git"

[tools.git.github]
owner = "git-for-windows"
repo = "git"
```

## Sections

### `[workspace]`

| Key    | Description                         |
|--------|-------------------------------------|
| `name` | Workspace name, set by `devbox init` |

### `[environment]`

Arbitrary environment variables applied to every process spawned by
`devbox exec`. Missing sections and keys fall back to defaults. Explicit
variables override the v0.8 isolation defaults (e.g. `HOME`, `TMP`,
`NUGET_PACKAGES`, `DOTNET_ROOT`) that otherwise point into `.devbox`.

### `[services]` (v0.9)

A table of long-running processes started and supervised by `devbox up`. Each
key is the service name; its value describes how to run it:

| Key           | Description                                   |
|---------------|-----------------------------------------------|
| `command`     | Executable to run (required)                  |
| `args`        | Arguments passed to the executable            |
| `cwd`         | Working directory (relative to the workspace) |
| `env_file`    | External TOML file with an `[environment]` table (relative to the workspace) |
| `environment` | Extra environment variables for this service  |

`command` is the executable to launch, resolved like any program name: against
`PATH`, which includes the executable directories of installed tools. `cwd`
only sets the process working directory — it does not locate the executable.
To run a binary from an installed tool, register it (`devbox tools register` or
`devbox install`) and use its name as `command`, or point `command` at the full
path and pass the subcommand via `args`:

```toml
[services.caddy]
command = "caddy"
args = ["run"]
```

Services run inside the isolated DevBox environment with their output
appended to `.devbox/workspace/logs/<name>.log`. See [cli.md](cli.md) for
`devbox up`, `status`, `logs`, and `stop`.

### `env_file` (external environment)

A service can load its environment variables from a separate TOML file instead
of inlining them in `devbox.toml`:

```toml
[services.contract-oncall]
command = "dotnet"
args = ["run"]
cwd = "C:/repos/contract-oncall"
env_file = "./.devbox/workspace/configs/contract-oncall.toml"
```

The referenced file contains an `[environment]` table:

```toml
[environment]
ASPNETCORE_ENVIRONMENT = "Development"
ConnectionStrings__Main = "Server=localhost;Database=ContractOnCall;"
```

Every key/value pair is injected into the spawned process. Relative
`env_file` paths resolve against the workspace root. The merge order, lowest
to highest precedence, is:

1. `env_file` values
2. Inline `[services.<name>.environment]` (overrides `env_file`)
3. Runtime overrides (future)

If the file is missing or cannot be parsed, `devbox up` fails service startup
with a descriptive error. The file is only read, never modified. This works
naturally with ASP.NET Core's built-in environment variable configuration
(`ConnectionStrings__Main`, `Logging__LogLevel__Default`, etc.).

### `[tools]`

A table of tools `devbox install` can resolve, beyond the built-in manifest
(which currently ships `ripgrep`). Each key is the tool name; its value
describes its GitHub release assets:

| Key               | Description                                        |
|-------------------|----------------------------------------------------|
| `default_version` | Version used when `--version` is omitted           |
| `executable`      | Executable name inside the archive, e.g. `git`     |
| `asset`           | Asset filename template (default `{name}-{version}-{triple}.{ext}`) |
| `github.owner`    | GitHub owner of the repository, e.g. `git-for-windows` |
| `github.repo`     | GitHub repository name, e.g. `git`                 |

The download URL is derived as
`https://github.com/{owner}/{repo}/releases/download/{version}/{asset}`. Without
an `asset` template this is
`{name}-{version}-{triple}.{ext}`, so the tool must publish archives in that
exact naming scheme. Tools that use a different scheme can declare a custom
`asset` template with the placeholders `{name}`, `{version}`, `{version_v}`,
`{os}` (GOOS: `windows`, `linux`, `darwin`), `{arch}` (GOARCH: `amd64`,
`arm64`), `{triple}` (Rust target triple), and `{ext}`.

Inside an `asset` template, `{version}` drops a leading `v` tag prefix (so a
release tagged `v2.11.3` becomes `2.11.3`), while `{version_v}` keeps the exact
release tag. The release tag in the URL path always uses the full version. For
example, caddy tags releases `v2.11.3` but publishes
`caddy_2.11.3_windows_amd64.zip`:

```toml
[tools.caddy]
default_version = "v2.11.3"
executable = "caddy"
asset = "caddy_{version}_{os}_{arch}.{ext}"

[tools.caddy.github]
owner = "caddyserver"
repo = "caddy"
```

A `[tools]` entry overrides the built-in spec of the same name. See
[downloader.md](downloader.md).
