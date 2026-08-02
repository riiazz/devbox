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

### `devbox services`

Manages the `enabled` flag of services declared in the `[services]` section of
`devbox.toml`. Disabled services stay defined in the config but are skipped by
`devbox up`. Services default to enabled, so existing configs are unaffected
until a service is explicitly disabled.

#### `devbox services enable <name>`

Marks a service as enabled so `devbox up` starts it. No-op if already enabled.

#### `devbox services disable <name>`

Marks a service as disabled so `devbox up` skips it, writing `enabled = false`
to the service in `devbox.toml`. No-op if already disabled. Enabling again
removes the flag.

Examples:

    devbox services disable caddy

    devbox services enable caddy

### `devbox up [--service <name>...] [--log-lines <n>]`

Starts every service declared in the `[services]` section of `devbox.toml`
(v0.9). Each service runs inside the isolated DevBox environment (see
[runtime.md](runtime.md)) with stdout/stderr appended to
`.devbox/workspace/logs/<name>.log`, and its PID is recorded in
`.devbox/workspace/processes.toml`. Any previously supervised services are
stopped first.

`devbox up` supervises the services in the foreground and redraws a live
dashboard once a second:

    devbox running with pid: 3333

    | service    | status  | pid  | parent_pid | cpu | memory | listening      |
    | ---------- | ------- | ---- | ---------- | --- | ------ | -------------- |
    | caddy      | running | 1100 | 3333       | 3%  | 2kb    | localhost:2009 |
    | rbac       | running | 1231 | 3333       | 5%  | 3mb    | localhost:4041 |
    | rbac-child | running | 1235 | 1231       |     |        |                |

    logs:
    ---
    service: caddy
    ...

The table reports each service's PID and parent PID, its current CPU usage and
memory, and the ports it is listening on. The logs section tails the last
`--log-lines` lines (default 5) for the first five services — or only the
services named by `--service` (repeatable).

Pressing Ctrl+C — or closing the terminal — interrupts the dashboard, stops the
supervised services, and clears the supervisor state. The dashboard also exits
once every service has stopped on its own.

Process tree:

    DevBox
    ├── API
    ├── Redis
    └── OTel

Examples:

    devbox up

    devbox up --service caddy --log-lines 20

    devbox up --service caddy --service rbac

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
the supervisor state. With no names, stops every supervised service. Only PIDs
still confirmed to be children of the devbox process that spawned them are
terminated — a PID that has since been reused by another program is left alone
and simply pruned from the state.

Examples:

    devbox stop redis

    devbox stop

### `devbox clear-logs [name...]`

Truncates the log files written by `devbox up` in `.devbox/workspace/logs/`,
emptying them while keeping the files so future runs keep appending and the
services stay visible in `devbox logs`. With no names, clears every service's
log. Services with no log file are reported as nothing to clear.

Examples:

    devbox clear-logs api

    devbox clear-logs

