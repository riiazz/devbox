use std::path::{Path, PathBuf};

use crate::environment::Environment;

/// Environment variables isolated onto `.devbox` (version 0.8).
pub const HOME_VAR: &str = "HOME";
pub const TMP_VAR: &str = "TMP";
pub const TEMP_VAR: &str = "TEMP";
pub const TMPDIR_VAR: &str = "TMPDIR";
pub const NUGET_PACKAGES_VAR: &str = "NUGET_PACKAGES";
pub const DOTNET_ROOT_VAR: &str = "DOTNET_ROOT";

/// The `.devbox` paths every spawned process is pointed at.
#[derive(Debug, Clone)]
pub struct Isolation {
    pub home: PathBuf,
    pub tmp: PathBuf,
    pub nuget_packages: PathBuf,
    pub dotnet_root: PathBuf,
}

impl Isolation {
    /// Standard isolation layout under a `.devbox` directory.
    pub fn from_devbox(devbox: &Path) -> Self {
        Self {
            home: devbox.join("home"),
            tmp: devbox.join("tmp"),
            nuget_packages: devbox.join("cache").join("nuget").join("packages"),
            dotnet_root: devbox.join("tools").join("dotnet"),
        }
    }

    /// Points the home/temp/package variables into `.devbox`.
    ///
    /// Callers may adjust `dotnet_root` before applying, e.g. to target a
    /// registered `dotnet` tool's install directory.
    pub fn apply(&self, env: &mut Environment) {
        env.set(HOME_VAR, path_str(&self.home));
        env.set(TMP_VAR, path_str(&self.tmp));
        env.set(TEMP_VAR, path_str(&self.tmp));
        env.set(TMPDIR_VAR, path_str(&self.tmp));
        env.set(NUGET_PACKAGES_VAR, path_str(&self.nuget_packages));
        env.set(DOTNET_ROOT_VAR, path_str(&self.dotnet_root));
    }
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_devbox_layout() {
        let iso = Isolation::from_devbox(Path::new(".devbox"));
        assert_eq!(iso.home, PathBuf::from(".devbox").join("home"));
        assert_eq!(iso.tmp, PathBuf::from(".devbox").join("tmp"));
        assert_eq!(
            iso.nuget_packages,
            PathBuf::from(".devbox").join("cache").join("nuget").join("packages")
        );
        assert_eq!(iso.dotnet_root, PathBuf::from(".devbox").join("tools").join("dotnet"));
    }

    #[test]
    fn apply_points_everything_into_devbox() {
        let mut env = Environment::from_current();
        let iso = Isolation::from_devbox(Path::new(".devbox"));
        iso.apply(&mut env);

        assert_eq!(env.get(HOME_VAR), Some(iso.home.to_string_lossy().into_owned()));
        assert_eq!(env.get(TMP_VAR), Some(iso.tmp.to_string_lossy().into_owned()));
        assert_eq!(env.get(TEMP_VAR), Some(iso.tmp.to_string_lossy().into_owned()));
        assert_eq!(env.get(TMPDIR_VAR), Some(iso.tmp.to_string_lossy().into_owned()));
        assert_eq!(
            env.get(NUGET_PACKAGES_VAR),
            Some(iso.nuget_packages.to_string_lossy().into_owned())
        );
        assert_eq!(
            env.get(DOTNET_ROOT_VAR),
            Some(iso.dotnet_root.to_string_lossy().into_owned())
        );
    }
}
