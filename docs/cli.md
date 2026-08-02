# CLI

## Commands

### `devbox exec <program> [args...]`

Runs `<program>` inside the DevBox environment. Applies the
`[environment]` section of `devbox.toml` if present, then prepends the
executable directories of every registered tool to `PATH`, so installed tools
resolve from `.devbox/tools` ahead of the system (v0.6).

Examples:

    devbox exec dotnet --info

    devbox exec cargo build

    devbox exec rg --version

Exits with the exit code of the spawned program.

### `devbox shell`

Opens an interactive shell inside the DevBox environment. Applies the same
environment as `devbox exec` — `[environment]` from `devbox.toml` plus the
installed tools on `PATH` — then spawns the user's shell (`$SHELL`, falling
back to PowerShell on Windows and `bash` on Unix), waits for it to finish, and
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
in the current directory. Idempotent. See [workspace.md](workspace.md) and
[config.md](config.md).

### `devbox install <name> [--version <version>]`

Resolves, downloads, verifies, extracts, and registers a tool. Version
defaults to the tool's default version in the manifest. Requires an
initialized workspace. See [downloader.md](downloader.md).

Examples:

    devbox install ripgrep

    devbox install ripgrep --version 14.1.0

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
