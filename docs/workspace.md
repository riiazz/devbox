# Workspace

Version 0.2 introduces the `.devbox/` workspace. It is a plain directory
tree owned by DevBox. No downloads yet — only structure.

## Layout

    <project>/
    └── .devbox/
        ├── workspace/
        │   ├── logs/
        │   └── processes.toml
        ├── cache/
        │   └── nuget/packages/
        ├── tools/
        ├── tmp/
        └── home/

## Commands

### `devbox init`

Creates the `.devbox/` directory tree in the current directory. Idempotent:
running it again is a no-op.

## Discovery

Commands that need a workspace locate it by walking up from the current
directory: the first ancestor containing a `.devbox/` directory wins. If no
such directory is found, DevBox falls back to a workspace created in the
directory that holds the `devbox` binary itself. This makes a global
environment usable from anywhere: `devbox init` next to the binary once, and
every command finds it no matter the current directory.

## Paths

| Directory  | Purpose                            |
|------------|------------------------------------|
| `workspace`| project files DevBox manages       |
| `cache`    | downloaded tool archives, `nuget/packages` (v0.8) |
| `tools`    | tool installs and `registry.toml`  |
| `tmp`      | temporary files for processes      |
| `home`     | isolated `HOME` (v0.8)             |

The workspace directory also holds the process supervisor's state and logs
(v0.9): `logs/` for per-service output and `processes.toml` for running PIDs.
