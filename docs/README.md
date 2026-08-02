# DevBox Documentation

DevBox is a reproducible development environment tool written in Rust.

## Status

Version 0.8

- CLI parser
- Runtime
- Environment Builder
- Process spawning
- Workspace (`devbox init`, `.devbox/` directory tree)
- Config (`devbox.toml`, `[workspace]` and `[environment]`)
- Tool Registry (`Tool`, registry, `devbox tools`)
- Downloader (`devbox install ripgrep`, Resolve → Download → Checksum → Extract → Register)
- Tool Resolution (`devbox exec rg` finds `.devbox/tools`, isolated PATH)
- Shell (`devbox shell`, interactive PowerShell/bash inside the environment)
- Environment Isolation (`HOME`, `TMP`, `NUGET_PACKAGES`, `DOTNET_ROOT` point into `.devbox`)

## Documents

| Doc | Description |
|-----|-------------|
| [cli.md](cli.md) | Command-line interface |
| [runtime.md](runtime.md) | Runtime, environment, process |
| [workspace.md](workspace.md) | `.devbox/` workspace |
| [config.md](config.md) | `devbox.toml` configuration |
| [toolchain.md](toolchain.md) | Tool registry |
| [downloader.md](downloader.md) | Tool download and install pipeline |
| [Architecture.md](Architecture.md) | Target architecture |
| [Principles.md](Principles.md) | Design principles |
| [Roadmap.md](Roadmap.md) | Versioned roadmap |
