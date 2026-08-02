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
| `environment` | Extra environment variables for this service  |

Services run inside the isolated DevBox environment with their output
appended to `.devbox/workspace/logs/<name>.log`. See [cli.md](cli.md) for
`devbox up`, `status`, `logs`, and `stop`.
