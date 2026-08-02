use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use toolchain::{Tool, ToolRegistry};

use crate::checksum::Checksum;
use crate::download::{self, DownloadError};
use crate::extract::{self, ArchiveFormat, ExtractError};
use crate::resolve::{resolve_source, Manifest, Source};
use crate::target::Target;

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("unknown tool `{0}`")]
    UnknownTool(String),
    #[error(transparent)]
    Resolve(#[from] crate::resolve::ResolveError),
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Extract(#[from] ExtractError),
    #[error("executable `{name}` not found in `{dir}`")]
    ExecutableNotFound { name: String, dir: PathBuf },
    #[error("failed to write cache archive `{path}`: {source}")]
    CacheWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read cache archive `{path}`: {source}")]
    CacheRead {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Downloads, verifies, extracts, and registers tools into a workspace.
pub struct Installer {
    client: reqwest::blocking::Client,
    install_root: PathBuf,
    cache_dir: PathBuf,
    manifest: Manifest,
}

impl Installer {
    pub fn new(install_root: impl Into<PathBuf>, cache_dir: impl Into<PathBuf>) -> Self {
        Self::with_manifest(install_root, cache_dir, Manifest::builtin())
    }

    /// An installer that resolves tools against `manifest` (built-in specs
    /// plus any user-defined `[tools]` entries).
    pub fn with_manifest(
        install_root: impl Into<PathBuf>,
        cache_dir: impl Into<PathBuf>,
        manifest: Manifest,
    ) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            install_root: install_root.into(),
            cache_dir: cache_dir.into(),
            manifest,
        }
    }

    /// Resolves a named tool and runs the full pipeline.
    pub fn install(
        &self,
        registry: &mut ToolRegistry,
        name: &str,
        version: Option<&str>,
    ) -> Result<Tool, InstallError> {
        let spec = self
            .manifest
            .resolve(name)
            .ok_or_else(|| InstallError::UnknownTool(name.to_string()))?;
        let version = version.unwrap_or(&spec.default_version);
        let source = resolve_source(spec, version, &Target::current())?;
        self.install_source(registry, name, version, &spec.executable, &source)
    }

    /// Runs the pipeline against an explicit source (used by tests).
    pub fn install_source(
        &self,
        registry: &mut ToolRegistry,
        name: &str,
        version: &str,
        executable: &str,
        source: &Source,
    ) -> Result<Tool, InstallError> {
        let archive_path = self.cache_path(name, version, source.format);
        let bytes = self.fetch_or_cache(&archive_path, source)?;

        let install_dir = self.install_root.join(name).join(version);
        extract::extract(&bytes, source.format, &install_dir)?;

        verify_executable(&install_dir, executable)?;

        let tool = Tool::new(name, version, executable, install_dir);
        registry.register(tool.clone());
        Ok(tool)
    }

    fn fetch_or_cache(&self, archive_path: &Path, source: &Source) -> Result<Vec<u8>, InstallError> {
        if archive_path.is_file() {
            let cached = fs::read(archive_path).map_err(|source| InstallError::CacheRead {
                path: archive_path.to_path_buf(),
                source,
            })?;
            if let Some(expected) = &source.checksum {
                match download::verify(&cached, expected, &source.url) {
                    Ok(()) => return Ok(cached),
                    // Stale or corrupted cache: fall through and refetch.
                    Err(DownloadError::ChecksumMismatch { .. }) => {}
                    Err(err) => return Err(err.into()),
                }
            } else {
                return Ok(cached);
            }
        }

        let bytes = download::fetch(&self.client, &source.url)?;
        if let Some(expected) = &source.checksum {
            download::verify(&bytes, expected, &source.url)?;
        }
        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).map_err(|source| InstallError::CacheWrite {
                path: archive_path.to_path_buf(),
                source,
            })?;
        }
        fs::write(archive_path, &bytes).map_err(|source| InstallError::CacheWrite {
            path: archive_path.to_path_buf(),
            source,
        })?;
        eprintln!("devbox: downloaded {} (sha256 {})", source.url, Checksum::compute(&bytes));
        Ok(bytes)
    }

    fn cache_path(&self, name: &str, version: &str, format: ArchiveFormat) -> PathBuf {
        let ext = match format {
            ArchiveFormat::Zip => "zip",
            ArchiveFormat::TarGz => "tar.gz",
        };
        self.cache_dir.join(format!("{name}-{version}.{ext}"))
    }
}

fn verify_executable(install_dir: &Path, executable: &str) -> Result<(), InstallError> {
    let exe_name = toolchain::path::executable_name(executable);
    if toolchain::path::find_file(install_dir, &exe_name).is_some() {
        Ok(())
    } else {
        Err(InstallError::ExecutableNotFound {
            name: exe_name,
            dir: install_dir.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;

    use flate2::write::GzEncoder;
    use flate2::Compression;
    use zip::write::SimpleFileOptions;

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let mut dir = std::env::temp_dir();
        dir.push(format!("devbox-installer-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn exe_name() -> &'static str {
        if cfg!(windows) {
            "rg.exe"
        } else {
            "rg"
        }
    }

    fn zip_fixture() -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file(exe_name(), SimpleFileOptions::default())
            .expect("start file");
        writer.write_all(b"fake binary").expect("write file");
        writer.finish().expect("finish zip").into_inner()
    }

    fn tar_gz_fixture() -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            let mut header = tar::Header::new_gnu();
            header.set_size(b"fake binary".len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, exe_name(), &b"fake binary"[..])
                .expect("append file");
            builder.finish().expect("finish tar");
        }
        encoder.finish().expect("finish gzip")
    }

    /// Serves `bytes` over a local HTTP server and returns its URL.
    fn serve(bytes: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local addr");
        thread::spawn(move || {
            for _ in 0..10 {
                let (mut stream, _) = match listener.accept() {
                    Ok(conn) => conn,
                    Err(_) => break,
                };
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&bytes);
            }
        });
        format!("http://{addr}/fixture")
    }

    #[test]
    fn installs_zip_and_registers() {
        let root = temp_dir();
        let tools = root.join("tools");
        let cache = root.join("cache");
        let mut registry = ToolRegistry::new();
        let installer = Installer::new(&tools, &cache);

        let source = Source {
            url: serve(zip_fixture()),
            format: ArchiveFormat::Zip,
            checksum: None,
        };
        installer
            .install_source(&mut registry, "ripgrep", "14.1.0", "rg", &source)
            .expect("install");

        let tool = registry.get_exact("ripgrep", "14.1.0").expect("registered");
        assert_eq!(tool.executable, "rg");
        assert!(tool.install_dir.join(exe_name()).is_file());
        assert!(cache.join("ripgrep-14.1.0.zip").is_file());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn installs_tar_gz() {
        let root = temp_dir();
        let mut registry = ToolRegistry::new();
        let installer = Installer::new(root.join("tools"), root.join("cache"));

        let source = Source {
            url: serve(tar_gz_fixture()),
            format: ArchiveFormat::TarGz,
            checksum: None,
        };
        installer
            .install_source(&mut registry, "ripgrep", "14.1.0", "rg", &source)
            .expect("install");
        assert!(registry.get("ripgrep").is_some());
        assert!(cache_has_archive(&root.join("cache"), "ripgrep-14.1.0.tar.gz"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn verifies_checksum_on_download() {
        let root = temp_dir();
        let mut registry = ToolRegistry::new();
        let installer = Installer::new(root.join("tools"), root.join("cache"));
        let fixture = zip_fixture();

        let good = Source {
            url: serve(fixture.clone()),
            format: ArchiveFormat::Zip,
            checksum: Some(Checksum::compute(&fixture)),
        };
        installer
            .install_source(&mut registry, "ripgrep", "14.1.0", "rg", &good)
            .expect("checksum matches");

        let bad = Source {
            url: serve(fixture.clone()),
            format: ArchiveFormat::Zip,
            checksum: Some(Checksum::compute(b"wrong")),
        };
        let err = installer
            .install_source(&mut registry, "fd", "9.0.0", "fd", &bad)
            .expect_err("checksum mismatch");
        assert!(matches!(err, InstallError::Download(DownloadError::ChecksumMismatch { .. })));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reuses_cache_when_checksum_matches() {
        let root = temp_dir();
        let mut registry = ToolRegistry::new();
        let installer = Installer::new(root.join("tools"), root.join("cache"));
        let fixture = zip_fixture();
        let checksum = Checksum::compute(&fixture);

        let source = Source {
            url: serve(fixture),
            format: ArchiveFormat::Zip,
            checksum: Some(checksum),
        };
        installer
            .install_source(&mut registry, "ripgrep", "14.1.0", "rg", &source)
            .expect("first install");
        installer
            .install_source(&mut registry, "ripgrep", "14.1.0", "rg", &source)
            .expect("second install (cached)");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn unknown_tool_errors() {
        let root = temp_dir();
        let mut registry = ToolRegistry::new();
        let installer = Installer::new(root.join("tools"), root.join("cache"));
        let err = installer
            .install(&mut registry, "not-a-real-tool", None)
            .expect_err("unknown tool");
        assert!(matches!(err, InstallError::UnknownTool(_)));
        fs::remove_dir_all(&root).ok();
    }

    fn cache_has_archive(cache: &Path, name: &str) -> bool {
        cache.join(name).is_file()
    }
}
