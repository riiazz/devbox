# DevBox Documentation

DevBox is a reproducible development environment tool written in Rust.

## Status

Version 0.4

- CLI parser
- Runtime
- Environment Builder
- Process spawning
- Workspace (`devbox init`, `.devbox/` directory tree)
- Config (`devbox.toml`, `[workspace]` and `[environment]`)
- Tool Registry (`Tool`, registry, `devbox tools`, no downloads yet)

## Documents

| Doc | Description |
|-----|-------------|
| [cli.md](cli.md) | Command-line interface |
| [runtime.md](runtime.md) | Runtime, environment, process |
| [workspace.md](workspace.md) | `.devbox/` workspace |
| [config.md](config.md) | `devbox.toml` configuration |
| [toolchain.md](toolchain.md) | Tool registry |
| [Architecture.md](Architecture.md) | Target architecture |
| [Principles.md](Principles.md) | Design principles |
| [Roadmap.md](Roadmap.md) | Versioned roadmap |
