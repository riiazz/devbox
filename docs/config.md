# Config

Version 0.3 introduces `devbox.toml` at the workspace root.

## Example

```toml
[workspace]
name = "Planning"

[environment]
DOTNET_ENVIRONMENT = "Development"
```

## Sections

### `[workspace]`

| Key    | Description                         |
|--------|-------------------------------------|
| `name` | Workspace name, set by `devbox init` |

### `[environment]`

Arbitrary environment variables applied to every process spawned by
`devbox exec`. Missing sections and keys fall back to defaults. Explicit
variables override the v0.8 isolation defaults (e.g. `HOME`, `TMP`,
`NUGET_PACKAGES`, `DOTNET_ROOT`) that otherwise point into `.devbox`.
