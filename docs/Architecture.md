
                    CLI
                     │
             Command Dispatcher
                     │
      ┌──────────────┼──────────────┐
      │              │              │
 Workspace      Tool Manager    Process Manager
      │              │              │
      └──────────────┼──────────────┘
                     │
                 Runtime
                     │
              Platform Layer
         Windows Linux macOS

---

Project structure

```markdown
    devbox/
    │
    ├── Cargo.toml
    ├── Cargo.lock
    ├── README.md
    ├── LICENSE
    ├── .gitignore
    │
    ├── docs/
    │   ├── vision.md
    │   ├── roadmap.md
    │   ├── architecture.md
    │   ├── runtime.md
    │   ├── workspace.md
    │   ├── toolchain.md
    │   ├── process.md
    │   ├── plugin.md
    │   ├── platform.md
    │   ├── configuration.md
    │   ├── security.md
    │   ├── glossary.md
    │   │
    │   └── adr/
    │       ├── 0001-project-goals.md
    │       ├── 0002-runtime-first.md
    │       ├── 0003-platform-abstraction.md
    │       ├── 0004-tool-installation.md
    │       └── template.md
    │
    ├── crates/
    │   ├── cli/
    │   ├── runtime/
    │   ├── workspace/
    │   ├── config/
    │   ├── toolchain/
    │   ├── downloader/
    │   ├── process/
    │   ├── proxy/
    │   ├── platform/
    │   ├── plugin/
    │   └── common/
    │
    ├── examples/
    │
    ├── tests/
    │
    └── scripts/
```
