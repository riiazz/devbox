# Changelog

All notable changes to DevBox are documented in this file. DevBox follows
[Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-08-02

DevBox is now a stable, reproducible development environment tool. The full
pipeline — workspace, configuration, tool install, isolated runtime, and
process supervision — works end to end.

### Added

- **`devbox init` is idempotent** — a second run is a no-op and leaves an
  existing `devbox.toml` untouched (v0.10.1).
- **`[tools]` configuration** — `devbox install` resolves user-defined tools
  from `devbox.toml`, overlaid on the built-in manifest (v0.10).
- **Stable CLI surface** — `exec`, `shell`, `init`, `install`, `tools`,
  `up`, `status`, `logs`, and `stop`.

### Features (per version)

The journey from 0.1 to 1.0:

- **0.1** — CLI parser, runtime, environment builder, process spawning.
  `devbox exec dotnet --info` works.
- **0.2** — Workspace: `devbox init` creates the `.devbox/` directory tree.
- **0.3** — Config: `devbox.toml` with `[workspace]` and `[environment]`.
- **0.4** — Tool registry: `Tool`, `registry.toml`, `devbox tools`.
- **0.5** — Downloader: `devbox install ripgrep` — Resolve → Download →
  Checksum → Extract → Register.
- **0.6** — Tool resolution: `devbox exec rg` finds `.devbox/tools`, isolated
  `PATH`.
- **0.7** — Shell: interactive PowerShell/bash inside the environment.
- **0.8** — Environment isolation: `HOME`, `TMP`, `NUGET_PACKAGES`,
  `DOTNET_ROOT` point into `.devbox`.
- **0.9** — Process supervisor: `devbox up`, `status`, `logs`, `stop` for
  `[services]`.
- **0.10** — `devbox install` resolves tools through the `[tools]` config.
- **0.10.1** — `devbox init` is idempotent.

### Supported platforms

- Windows (PowerShell)
- Linux (bash)
- macOS (bash)

## [0.10.1] - 2026

- Make `devbox init` idempotent.

## [0.10] - 2026

- Resolve `devbox install` through the `[tools]` section of `devbox.toml`.

## [0.9] - 2026

- Process supervisor: `devbox up`, `status`, `logs`, `stop`.

## [0.8] - 2026

- Environment isolation: `HOME`, `TMP`, `NUGET_PACKAGES`, `DOTNET_ROOT`.

## [0.7] - 2026

- `devbox shell` — interactive shell inside the environment.

## [0.6] - 2026

- Tool resolution — isolated `PATH` via the tool registry.

## [0.5] - 2026

- Downloader: `devbox install <name>` — Resolve → Download → Checksum →
  Extract → Register.

## [0.4] - 2026

- Tool registry: `Tool`, `.devbox/tools/registry.toml`, `devbox tools`.

## [0.3] - 2026

- Config: `devbox.toml` with `[workspace]` and `[environment]`.

## [0.2] - 2026

- Workspace: `devbox init` creates the `.devbox/` directory tree.

## [0.1] - 2026

- CLI parser, runtime, environment builder, process spawning.

[1.0.0]: https://github.com/devbox/devbox/releases/tag/v1.0.0
