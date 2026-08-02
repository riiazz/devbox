# Downloader

Version 0.5 makes `devbox install <name>` work. Tools are resolved, downloaded,
verified, extracted, and registered — nothing is done by hand.

    Resolve
        ↓
    Download
        ↓
    Checksum
        ↓
    Extract
        ↓
    Register

## Pipeline

1. **Resolve** — a tool name is looked up in the built-in manifest
   (`crates/downloader/src/resolve.rs`) and mapped to a concrete download
   source. Sources are per-platform GitHub releases; the target triple is
   derived from the running OS and architecture.

2. **Download** — the archive is fetched with `reqwest` into
   `.devbox/cache/<name>-<version>.<ext>`. A matching cached archive is reused.

3. **Checksum** — a `sha256` is computed over the downloaded bytes. If the
   source declares a checksum it is verified; on mismatch the download fails
   and a stale cache is refetched.

4. **Extract** — the archive is unpacked into
   `.devbox/tools/<name>/<version>/`. Both `zip` and `tar.gz` archives are
   supported, and path-traversal entries are rejected.

5. **Register** — the executable is located inside the install directory and a
   `Tool` is added to `.devbox/tools/registry.toml` (see
   [toolchain.md](toolchain.md)).

## Commands

See [cli.md](cli.md) for `devbox install`.

## Roadmap

Version 0.6 uses the registry to point `PATH` at installed tools, so
`devbox exec rg` resolves to `.devbox/tools/...` instead of the system path.
