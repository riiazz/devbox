# DevBox

Reproducible development environments, written in Rust.

DevBox owns the development environment — not the operating system. It
integrates with the tools you already use instead of replacing them, and it is
reproducible: clone, `devbox init`, `devbox shell`, done.

```
    git clone <project>

    ↓

    devbox init

    ↓

    devbox shell
```

## Features

- **Isolated runtime** — `HOME`, `TMP`, `NUGET_PACKAGES`, and `DOTNET_ROOT`
  point into `.devbox/`; your system environment is never modified.
- **Tool installs** — `devbox install ripgrep` resolves, downloads, checksums,
  extracts, and registers tools from GitHub releases.
- **Isolated PATH** — `devbox exec rg` finds `.devbox/tools` ahead of the
  system, so your project tools always win.
- **Interactive shell** — `devbox shell` drops you into a PowerShell/bash
  session inside the environment.
- **Process supervision** — declare `[services]` in `devbox.toml` and manage
  them with `devbox up`, `status`, `logs`, and `stop`.

## Quick start

```text
cargo install --path crates/cli --locked

devbox init
devbox install ripgrep
devbox exec rg --version
```

## Commands

| Command | Description |
|---------|-------------|
| `devbox init` | Create the `.devbox/` workspace and a starter `devbox.toml` |
| `devbox exec <program> [args...]` | Run a program inside the DevBox environment |
| `devbox shell` | Open an interactive shell inside the environment |
| `devbox install <name> [--version <v>]` | Download and register a tool |
| `devbox tools list` / `devbox tools register` | Manage the tool registry |
| `devbox up` | Start and supervise `[services]` |
| `devbox status` | Report service PIDs and status |
| `devbox logs [name] [--lines <n>]` | Print service logs |
| `devbox stop [name...]` | Stop supervised services |

## Documentation

Full documentation lives in [docs/](docs/README.md) — commands, configuration,
architecture, and the versioned roadmap.

## Releases

- [CHANGELOG.md](CHANGELOG.md)
- [v1.0.0 release notes](docs/release/v1.0.0.md)

## Status

v1.0.0 — stable. Per the [roadmap](docs/Roadmap.md), v1.0 is where DevBox
becomes useful; the plan is to use it in anger before adding features.
