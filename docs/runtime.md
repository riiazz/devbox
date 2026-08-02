# Runtime

The runtime owns the environment of every process DevBox spawns.

    CLI
     │
    Runtime
     │
    std::process::Command

## Environment Builder

Starts from the current process environment, then applies:

- variable overrides (`set_var`, `unset_var`)
- PATH injection (`prepend_path`, `append_path`)

## Environment Isolation

Version 0.8 isolates the standard developer variables into `.devbox/`:

| Variable         | Points to                          |
|------------------|------------------------------------|
| `HOME`           | `.devbox/home/`                    |
| `TMP` `TEMP`     | `.devbox/tmp/`                     |
| `TMPDIR`         | `.devbox/tmp/`                     |
| `NUGET_PACKAGES` | `.devbox/cache/nuget/packages/`    |
| `DOTNET_ROOT`    | `.devbox/tools/dotnet/` (or a registered `dotnet` tool's install dir) |
| `PATH`           | installed tool dirs (v0.6)         |

Explicit variables in `[environment]` from `devbox.toml` override these
defaults. The target directories are created on demand.

## Process spawning

`Runtime::exec` builds a `std::process::Command`, applies the environment,
spawns the child with inherited stdio, and waits for it to finish.
