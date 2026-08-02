/// The platform DevBox targets when resolving tool archives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub os: &'static str,
    pub arch: &'static str,
}

impl Target {
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }

    /// Rust target triple for which upstream provides archives.
    pub fn triple(&self) -> Option<&'static str> {
        match (self.os, self.arch) {
            ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
            ("windows", "aarch64") => Some("aarch64-pc-windows-msvc"),
            ("linux", "x86_64") => Some("x86_64-unknown-linux-musl"),
            ("linux", "aarch64") => Some("aarch64-unknown-linux-musl"),
            ("macos", "x86_64") => Some("x86_64-apple-darwin"),
            ("macos", "aarch64") => Some("aarch64-apple-darwin"),
            _ => None,
        }
    }

    /// Whether upstream ships the archive as a zip or a tarball.
    pub fn archive_format(&self) -> crate::extract::ArchiveFormat {
        if self.os == "windows" {
            crate::extract::ArchiveFormat::Zip
        } else {
            crate::extract::ArchiveFormat::TarGz
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_has_os_and_arch() {
        let target = Target::current();
        assert!(!target.os.is_empty());
        assert!(!target.arch.is_empty());
    }

    #[test]
    fn supports_windows_x86_64() {
        let target = Target { os: "windows", arch: "x86_64" };
        assert_eq!(target.triple(), Some("x86_64-pc-windows-msvc"));
    }

    #[test]
    fn supports_linux_arm64() {
        let target = Target { os: "linux", arch: "aarch64" };
        assert_eq!(target.triple(), Some("aarch64-unknown-linux-musl"));
    }

    #[test]
    fn rejects_unknown_platform() {
        let target = Target { os: "fuchsia", arch: "x86_64" };
        assert_eq!(target.triple(), None);
    }
}
