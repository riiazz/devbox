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
#[derive(Debug, Clone, Copy)]
pub struct GitHubRepo {
    pub owner: &'static str,
    pub repo: &'static str,
}

/// Static description of a tool that DevBox knows how to install.
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub name: &'static str,
    pub default_version: &'static str,
    pub executable: &'static str,
    pub github: GitHubRepo,
}

/// Built-in tool manifest. Extended later by user/plugin manifests.
pub static TOOL_SPECS: &[ToolSpec] = &[ToolSpec {
    name: "ripgrep",
    default_version: "14.1.0",
    executable: "rg",
    github: GitHubRepo {
        owner: "BurntSushi",
        repo: "ripgrep",
    },
}];

pub fn resolve(name: &str) -> Option<&'static ToolSpec> {
    TOOL_SPECS.iter().find(|spec| spec.name == name)
}

/// Resolves a tool to a concrete download source for the given target.
pub fn resolve_source(
    spec: &ToolSpec,
    version: &str,
    target: &Target,
) -> Result<Source, ResolveError> {
    let triple = target.triple().ok_or_else(|| ResolveError::UnsupportedTarget {
        tool: spec.name,
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
    UnsupportedTarget { tool: &'static str, target: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_finds_ripgrep() {
        let spec = resolve("ripgrep").expect("ripgrep in manifest");
        assert_eq!(spec.name, "ripgrep");
        assert_eq!(spec.executable, "rg");
        assert_eq!(spec.default_version, "14.1.0");
    }

    #[test]
    fn resolve_unknown_tool_is_none() {
        assert!(resolve("definitely-not-a-tool").is_none());
    }

    #[test]
    fn windows_uses_zip() {
        let spec = resolve("ripgrep").unwrap();
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
        let spec = resolve("ripgrep").unwrap();
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
        let spec = resolve("ripgrep").unwrap();
        let err = resolve_source(spec, "14.1.0", &Target { os: "fuchsia", arch: "x86_64" })
            .expect_err("unsupported");
        assert!(matches!(err, ResolveError::UnsupportedTarget { .. }));
    }
}
