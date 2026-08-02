# CLI

## Commands

### `devbox exec <program> [args...]`

Runs `<program>` inside the DevBox environment. Isolates `HOME`, `TMP`,
`NUGET_PACKAGES`, and `DOTNET_ROOT` into `.devbox` (v0.8), applies the
`[environment]` section of `devbox.toml` if present (overriding the isolated
defaults), then prepends the executable directories of every registered tool
to `PATH`, so installed tools resolve from `.devbox/tools` ahead of the system
(v0.6).

Examples:

    devbox exec dotnet --info

    devbox exec cargo build

    devbox exec rg --version

Exits with the exit code of the spawned program.

### `devbox shell`

Opens an interactive shell inside the DevBox environment. Applies the same
environment as `devbox exec` — isolated `HOME`/`TMP`/`NUGET_PACKAGES`/
`DOTNET_ROOT` (v0.8), `[environment]` from `devbox.toml`, plus the installed
tools on `PATH` — then spawns the user's shell (`$SHELL`, falling back to
PowerShell on Windows and `bash` on Unix), waits for it to finish, and
propagates its exit code. No system environment variables are modified; the
environment lives only in the child process.

Pipeline:

    Create ENV
        ↓
    Spawn shell
        ↓
    Wait
        ↓
    Cleanup (nothing persists)

### `devbox init`

Creates the `.devbox/` workspace directory tree and a starter `devbox.toml`
in the current directory. Idempotent: a second run is a no-op and leaves an
existing `devbox.toml` untouched. See [workspace.md](workspace.md) and
[config.md](config.md).

### `devbox install <name> [--version <version>]`

Resolves, downloads, verifies, extracts, and registers a tool. Version
defaults to the tool's default version. The tool must be in the built-in
manifest or declared in the `[tools]` section of `devbox.toml` (see
[config.md](config.md)). Requires an initialized workspace. See
[downloader.md](downloader.md).

Examples:

    devbox install ripgrep

    devbox install ripgrep --version 14.1.0

    devbox install git

### `devbox tools`

Manages the tool registry stored at `.devbox/tools/registry.toml`. No
downloads yet (version 0.5). See [toolchain.md](toolchain.md).

#### `devbox tools list`

Lists registered tools, ordered by name then version. Example:

    ripgrep 14.1.0 (rg at .devbox\tools\rg\14.1.0)

#### `devbox tools register <name> <version> --executable <exe> [--dir <dir>]`

Registers a tool manually. Replaces a tool with the same name and version.
`--dir` defaults to the registry directory (`.devbox/tools/`).
Requires an initialized workspace.

### `devbox up`

Starts every service declared in the `[services]` section of `devbox.toml`
(v0.9). Each service runs inside the isolated DevBox environment (see
[runtime.md](runtime.md)) with stdout/stderr appended to
`.devbox/workspace/logs/<name>.log`, and its PID is recorded in
`.devbox/workspace/processes.toml`. `devbox up` supervises the services in the
foreground, printing each exit as it happens, and exits once all services have
stopped. Any previously supervised services are stopped first.

Process tree:

    DevBox
    ├── API
    ├── Redis
    └── OTel

Example:

    devbox up

### `devbox status`

Reads the supervisor state and reports each service's PID and whether it is
still running. Example:

    NAME         PID  STATUS
    api         1234  running
    redis       5678  stopped

### `devbox logs [name] [--lines <n>]`

Prints the trailing log lines for a service (all services when `name` is
omitted, default 100 lines). Logs are the live files written by `devbox up`.

Examples:

    devbox logs api

    devbox logs redis --lines 50

### `devbox stop [name...]`

Stops supervised services, terminating each process tree and pruning it from
the supervisor state. With no names, stops every supervised service.

Examples:

    devbox stop redis

    devbox stop
