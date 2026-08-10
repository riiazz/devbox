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
    /// Asset filename template; falls back to `{name}-{version}-{triple}.{ext}`
    /// when `None`.
    pub asset: Option<String>,
    pub github: GitHubRepo,
}

impl From<(String, ToolConfig)> for ToolSpec {
    fn from((name, tool): (String, ToolConfig)) -> Self {
        Self {
            name,
            default_version: tool.default_version,
            executable: tool.executable,
            asset: tool.asset,
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
        asset: None,
        github: GitHubRepo {
            owner: "BurntSushi".into(),
            repo: "ripgrep".into(),
        },
    }]
}

/// The default asset template: `<name>-<version>-<triple>.<ext>`.
const DEFAULT_ASSET_TEMPLATE: &str = "{name}-{version}-{triple}.{ext}";

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
    let goos = target.goos().ok_or_else(|| ResolveError::UnsupportedTarget {
        tool: spec.name.clone(),
        target: format!("{}-{}", target.os, target.arch),
    })?;
    let goarch = target.goarch().ok_or_else(|| ResolveError::UnsupportedTarget {
        tool: spec.name.clone(),
        target: format!("{}-{}", target.os, target.arch),
    })?;
    let format = target.archive_format();
    let ext = match format {
        ArchiveFormat::Zip => "zip",
        ArchiveFormat::TarGz => "tar.gz",
    };
    let template = spec.asset.as_deref().unwrap_or(DEFAULT_ASSET_TEMPLATE);
    let asset = render(
        template,
        &[
            ("name", &spec.name),
            // In asset names `{version}` drops a leading `v` tag prefix;
            // `{version_v}` keeps the exact release tag.
            ("version", version_without_v(version)),
            ("version_v", version),
            ("os", goos),
            ("arch", goarch),
            ("triple", triple),
            ("ext", ext),
        ],
    );
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

/// Replaces `{key}` placeholders in `template` with their values.
fn render(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in values {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

/// The asset version: the release tag with a single leading `v` prefix
/// stripped when it is followed by a digit (e.g. `v2.11.3` -> `2.11.3`).
fn version_without_v(version: &str) -> &str {
    match version.strip_prefix('v') {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
        _ => version,
    }
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
            asset: None,
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
            asset: None,
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
            asset: Some("git-{version}-{triple}.{ext}".into()),
            github: GithubSource {
                owner: "git-for-windows".into(),
                repo: "git".into(),
            },
        };
        let spec = ToolSpec::from(("git".to_string(), tool));
        assert_eq!(spec.name, "git");
        assert_eq!(spec.github.owner, "git-for-windows");
        assert_eq!(spec.asset.as_deref(), Some("git-{version}-{triple}.{ext}"));
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

    #[test]
    fn custom_asset_template_uses_go_style_names() {
        let mut manifest = manifest();
        manifest.add(ToolSpec {
            name: "caddy".into(),
            default_version: "v2.11.3".into(),
            executable: "caddy".into(),
            asset: Some("caddy_{version}_{os}_{arch}.{ext}".into()),
            github: GitHubRepo {
                owner: "caddyserver".into(),
                repo: "caddy".into(),
            },
        });

        let spec = manifest.resolve("caddy").unwrap();
        let source = resolve_source(spec, "v2.11.3", &Target { os: "windows", arch: "x86_64" })
            .expect("resolve");
        assert_eq!(source.format, ArchiveFormat::Zip);
        assert_eq!(
            source.url,
            "https://github.com/caddyserver/caddy/releases/download/v2.11.3/\
             caddy_2.11.3_windows_amd64.zip"
        );
    }

    #[test]
    fn asset_template_can_keep_v_tag_prefix() {
        let mut manifest = manifest();
        manifest.add(ToolSpec {
            name: "helm".into(),
            default_version: "v3.16.0".into(),
            executable: "helm".into(),
            asset: Some("helm-{version_v}-{os}-{arch}.{ext}".into()),
            github: GitHubRepo {
                owner: "helm".into(),
                repo: "helm".into(),
            },
        });

        let spec = manifest.resolve("helm").unwrap();
        let source = resolve_source(spec, "v3.16.0", &Target { os: "linux", arch: "x86_64" })
            .expect("resolve");
        assert_eq!(source.format, ArchiveFormat::TarGz);
        assert_eq!(
            source.url,
            "https://github.com/helm/helm/releases/download/v3.16.0/\
             helm-v3.16.0-linux-amd64.tar.gz"
        );
    }

    #[test]
    fn default_asset_template_uses_rust_triple() {
        let mut manifest = manifest();
        manifest.add(ToolSpec {
            name: "caddy".into(),
            default_version: "2.11.3".into(),
            executable: "caddy".into(),
            asset: None,
            github: GitHubRepo {
                owner: "caddyserver".into(),
                repo: "caddy".into(),
            },
        });

        let spec = manifest.resolve("caddy").unwrap();
        let source = resolve_source(spec, "2.11.3", &Target { os: "windows", arch: "x86_64" })
            .expect("resolve");
        assert_eq!(
            source.url,
            "https://github.com/caddyserver/caddy/releases/download/2.11.3/\
             caddy-2.11.3-x86_64-pc-windows-msvc.zip"
        );
    }
}
