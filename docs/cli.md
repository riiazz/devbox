# CLI

Version 0.1 implements a single command.

## Usage

    devbox exec <program> [args...]

Runs `<program>` inside the DevBox environment.

## Examples

    devbox exec dotnet --info

    devbox exec cargo build

## Exit codes

`devbox exec` exits with the exit code of the spawned program.
