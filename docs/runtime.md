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

In version 0.1 the environment is inherited from the calling shell. Later
versions point `HOME`, `TMP`, `DOTNET_ROOT`, and `PATH` into `.devbox/`.

## Process spawning

`Runtime::exec` builds a `std::process::Command`, applies the environment,
spawns the child with inherited stdio, and waits for it to finish.
