use config::ToolConfig;
use thiserror::Error;

use crate::checksum::Checksum;
use crate::extract::ArchiveFormat;
use crate::target::Target;

/// A description of where to fetch a tool archive from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub url: String,
    pub format: ArchiveFormat,
    pub checksum: Option<Checksum>,
}

/// Where a tool ships its prebuilt binaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepo {
    pub owner: String,
    pub repo: String,
}

/// Description of a tool that DevBox knows how to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    pub default_version: String,
    pub executable: String,
    pub github: GitHubRepo,
}

impl From<(String, ToolConfig)> for ToolSpec {
    fn from((name, tool): (String, ToolConfig)) -> Self {
        Self {
            name,
            default_version: tool.default_version,
            executable: tool.executable,
            github: GitHubRepo {
                owner: tool.github.owner,
                repo: tool.github.repo,
            },
        }
    }
}

/// An ordered collection of tool specs: the built-in manifest plus any
/// user-defined `[tools]` entries, which override built-ins of the same name.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    specs: Vec<ToolSpec>,
}

impl Manifest {
    /// The tools DevBox ships with, plus nothing else.
    pub fn builtin() -> Self {
        Self {
            specs: builtin_specs(),
        }
    }

    /// Adds or replaces a spec by name.
    pub fn add(&mut self, spec: ToolSpec) {
        self.specs.retain(|existing| existing.name != spec.name);
        self.specs.push(spec);
    }

    /// Resolves a tool name to its spec.
    pub fn resolve(&self, name: &str) -> Option<&ToolSpec> {
        self.specs.iter().find(|spec| spec.name == name)
    }

    pub fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }
}

/// Built-in tool manifest. Extended by user `[tools]` entries in `devbox.toml`.
fn builtin_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "ripgrep".into(),
        default_version: "14.1.0".into(),
        executable: "rg".into(),
        github: GitHubRepo {
            owner: "BurntSushi".into(),
            repo: "ripgrep".into(),
        },
    }]
}

/// Resolves a tool to a concrete download source for the given target.
pub fn resolve_source(
    spec: &ToolSpec,
    version: &str,
    target: &Target,
) -> Result<Source, ResolveError> {
    let triple = target.triple().ok_or_else(|| ResolveError::UnsupportedTarget {
        tool: spec.name.clone(),
        target: format!("{}-{}", target.os, target.arch),
    })?;
    let format = target.archive_format();
    let ext = match format {
        ArchiveFormat::Zip => "zip",
        ArchiveFormat::TarGz => "tar.gz",
    };
    let asset = format!("{}-{}-{}.{}", spec.name, version, triple, ext);
    let url = format!(
        "https://github.com/{}/{}/releases/download/{}/{}",
        spec.github.owner, spec.github.repo, version, asset
    );
    Ok(Source {
        url,
        format,
        checksum: None,
    })
}

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("no supported download for `{tool}` on target `{target}`")]
    UnsupportedTarget { tool: String, target: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::GithubSource;

    fn manifest() -> Manifest {
        Manifest::builtin()
    }

    #[test]
    fn resolve_finds_ripgrep() {
        let m = manifest();
        let spec = m.resolve("ripgrep").expect("ripgrep in manifest");
        assert_eq!(spec.name, "ripgrep");
        assert_eq!(spec.executable, "rg");
        assert_eq!(spec.default_version, "14.1.0");
    }

    #[test]
    fn resolve_unknown_tool_is_none() {
        assert!(manifest().resolve("definitely-not-a-tool").is_none());
    }

    #[test]
    fn custom_spec_is_resolvable() {
        let mut manifest = manifest();
        manifest.add(ToolSpec {
            name: "git".into(),
            default_version: "2.45.0".into(),
            executable: "git".into(),
            github: GitHubRepo {
                owner: "git-for-windows".into(),
                repo: "git".into(),
            },
        });

        let spec = manifest.resolve("git").expect("custom spec");
        assert_eq!(spec.executable, "git");
        assert_eq!(spec.github.repo, "git");
    }

    #[test]
    fn custom_spec_overrides_builtin() {
        let mut manifest = manifest();
        manifest.add(ToolSpec {
            name: "ripgrep".into(),
            default_version: "99.0.0".into(),
            executable: "rg".into(),
            github: GitHubRepo {
                owner: "fork".into(),
                repo: "ripgrep".into(),
            },
        });

        let spec = manifest.resolve("ripgrep").expect("overridden spec");
        assert_eq!(spec.default_version, "99.0.0");
        assert_eq!(spec.github.owner, "fork");
    }

    #[test]
    fn from_config_builds_spec() {
        let tool = ToolConfig {
            default_version: "2.45.0".into(),
            executable: "git".into(),
            github: GithubSource {
                owner: "git-for-windows".into(),
                repo: "git".into(),
            },
        };
        let spec = ToolSpec::from(("git".to_string(), tool));
        assert_eq!(spec.name, "git");
        assert_eq!(spec.github.owner, "git-for-windows");
    }

    #[test]
    fn windows_uses_zip() {
        let m = manifest();
        let spec = m.resolve("ripgrep").unwrap();
        let source = resolve_source(spec, "14.1.0", &Target { os: "windows", arch: "x86_64" })
            .expect("resolve");
        assert_eq!(source.format, ArchiveFormat::Zip);
        assert_eq!(
            source.url,
            "https://github.com/BurntSushi/ripgrep/releases/download/14.1.0/\
             ripgrep-14.1.0-x86_64-pc-windows-msvc.zip"
        );
    }

    #[test]
    fn linux_uses_tar_gz() {
        let m = manifest();
        let spec = m.resolve("ripgrep").unwrap();
        let source = resolve_source(spec, "14.1.0", &Target { os: "linux", arch: "x86_64" })
            .expect("resolve");
        assert_eq!(source.format, ArchiveFormat::TarGz);
        assert_eq!(
            source.url,
            "https://github.com/BurntSushi/ripgrep/releases/download/14.1.0/\
             ripgrep-14.1.0-x86_64-unknown-linux-musl.tar.gz"
        );
    }

    #[test]
    fn unsupported_target_errors() {
        let m = manifest();
        let spec = m.resolve("ripgrep").unwrap();
        let err = resolve_source(spec, "14.1.0", &Target { os: "fuchsia", arch: "x86_64" })
            .expect_err("unsupported");
        assert!(matches!(err, ResolveError::UnsupportedTarget { .. }));
    }
}
