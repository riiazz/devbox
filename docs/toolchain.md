# Tool Registry

Version 0.4 introduces the tool registry. No downloads yet — only metadata.

## `Tool`

A registered tool is described by:

```rust
struct Tool {
    name,
    version,
    executable,
    install_dir,
}
```

| Field         | Description                              |
|---------------|------------------------------------------|
| `name`        | Tool name, e.g. `ripgrep`                |
| `version`     | Version, e.g. `14.1.0`                   |
| `executable`  | Executable name, e.g. `rg`               |
| `install_dir` | Directory the tool is installed into     |

Tools are keyed by `(name, version)`, so one tool can be registered at
multiple versions. Resolving by name returns the highest version.

## Registry

Stored at `.devbox/tools/registry.toml`:

```toml
[[tools]]
name = "ripgrep"
version = "14.1.0"
executable = "rg"
install_dir = ".devbox/tools/rg/14.1.0"
```

Operations: `register`, `unregister`, `get`, `get_exact`, `versions`, `list`,
`load`, `save`, `executable_dirs`.

`executable_dirs` returns the bin directory of each installed tool (highest
version per name first). Version 0.6 prepends these to `PATH` so `devbox exec
rg` finds `.devbox/tools` instead of the system path.

## Commands

See [cli.md](cli.md) for `devbox tools list` and `devbox tools register`.

## Roadmap

Version 0.5 added the downloader: `devbox install ripgrep` resolves, downloads,
checksums, extracts, and registers — the final "Register" step lands in this
registry. Version 0.6 completed: `devbox exec` uses `executable_dirs` to build
the isolated `PATH`. Version 0.7 completed: `devbox shell` reuses the same
environment. Version 0.8 completed: `HOME`, `TMP`, `DOTNET_ROOT`, and
`NUGET_PACKAGES` are isolated into `.devbox`.
