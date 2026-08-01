# CLI

## Commands

### `devbox exec <program> [args...]`

Runs `<program>` inside the DevBox environment.

Examples:

    devbox exec dotnet --info

    devbox exec cargo build

Exits with the exit code of the spawned program.

### `devbox init`

Creates the `.devbox/` workspace directory tree in the current directory.
Idempotent. See [workspace.md](workspace.md).
