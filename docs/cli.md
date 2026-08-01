# CLI

## Commands

### `devbox exec <program> [args...]`

Runs `<program>` inside the DevBox environment. Applies the
`[environment]` section of `devbox.toml` if present.

Examples:

    devbox exec dotnet --info

    devbox exec cargo build

Exits with the exit code of the spawned program.

### `devbox init`

Creates the `.devbox/` workspace directory tree and a starter `devbox.toml`
in the current directory. Idempotent. See [workspace.md](workspace.md) and
[config.md](config.md).
