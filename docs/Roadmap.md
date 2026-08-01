Version 0.1
    Implement

    CLI parser
    Runtime
    Environment Builder
    Process spawning

    You should learn

    std::process::Command
    env vars
    PATH injection

    Deliver:
    devbox exec dotnet --info
---

Version 0.2
    Workspace

    Introduce

    ```markdown
        .devbox/
        workspace/
        cache/
        tools/
        tmp/
        home/
    ```

    Commands

    `devbox init`

    creates

    ```markdown
        workspace/
        .devbox/
    ```

    No downloads yet.

    Only directory structure.
---

Version 0.3
    Config

    Introduce

    devbox.toml
    [workspace]
    name = "Planning"

    [environment]
    DOTNET_ENVIRONMENT = "Development"

    Learn

    serde
    toml
---

Version 0.4
    Tool Registry

    Design
    ```rust
        Tool
    ```

    ```rust
        struct Tool {
            name,
            version,
            executable,
            install_dir,
        }
    ```

    No downloads yet.

    Just registry.
---

Version 0.5
    Downloader

    Now

    `devbox install ripgrep`

    works.

    Pipeline

    ```markdown
        Resolve
        ↓
        Download
        ↓
        Checksum
        ↓
        Extract
        ↓
        Register
    ```

    Learn

    reqwest
    zip
    tar
    sha256

---

Version 0.6
    Tool Resolution

    `devbox exec rg`

    should automatically locate

    `.devbox/tools/rg`

    instead of

    `Program Files`

    Now you've created isolated PATH.
---

Version 0.7
    Shell

    `devbox shell`

    Pipeline

    ```markdown
        Create ENV
        ↓
        Spawn PowerShell
        ↓
        Wait
        ↓
        Cleanup
    ```

    This is where DevBox starts feeling magical.
---

Version 0.8
    Environment Isolation

    Support

    ```markdown
        HOME
        TMP
        PATH
        NUGET_PACKAGES
        DOTNET_ROOT
        Everything points into
        .devbox
    ```
---

Version 0.9
    Process Supervisor
    `devbox up`

    reads

    ```markdown
        [services]
        api
        redis
        otel
    ```

    Creates

    ```markdown
        DevBox
        ├── API
        ├── Redis
        └── OTel
    ```

    Commands

    ```markdown
        logs
        status
        stop
    ```
---

Version 1.0

    Congratulations.
    You now have something useful.
    I'd stop.
    Seriously.
    Use it for months.
    Don't add features.
    You'll discover what's actually missing.
---

Additionals

    Rust: crates I'd use
    Purpose: Crate
    CLI: clap
    Config: serde, toml
    Async: tokio
    Download: reqwest
    Logging: tracing
    Errors: thiserror, anyhow
    Zip: zip
    Tar: tar, flate2
    SHA256: sha2
    Paths: camino (optional, nice ergonomics)
    Progress: bars	indicatif
    Process: management	tokio::process (or std::process initially)
