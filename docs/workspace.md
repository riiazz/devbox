# Workspace

Version 0.2 introduces the `.devbox/` workspace. It is a plain directory
tree owned by DevBox. No downloads yet — only structure.

## Layout

    <project>/
    └── .devbox/
        ├── workspace/
        ├── cache/
        ├── tools/
        ├── tmp/
        └── home/

## Commands

### `devbox init`

Creates the `.devbox/` directory tree in the current directory. Idempotent:
running it again is a no-op.

## Paths

| Directory  | Purpose                            |
|------------|------------------------------------|
| `workspace`| project files DevBox manages       |
| `cache`    | downloaded tool archives (v0.5+)   |
| `tools`    | extracted tool installs (v0.5+)    |
| `tmp`      | temporary files for processes      |
| `home`     | isolated `HOME` (v0.8)             |
