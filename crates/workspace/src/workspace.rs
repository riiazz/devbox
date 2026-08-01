use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub const DEVBOX_DIR: &str = ".devbox";

pub const SUBDIRS: [&str; 5] = ["workspace", "cache", "tools", "tmp", "home"];

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("failed to create `{path}`: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("`{0}` is not a devbox workspace (run `devbox init`)")]
    NotInitialized(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn init(root: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        let root = root.into();
        let devbox = root.join(DEVBOX_DIR);
        for subdir in std::iter::once(devbox.clone()).chain(SUBDIRS.iter().map(|s| devbox.join(s))) {
            fs::create_dir_all(&subdir).map_err(|source| WorkspaceError::CreateDir {
                path: subdir.clone(),
                source,
            })?;
        }
        Ok(Self { root })
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        let root = root.into();
        if !root.join(DEVBOX_DIR).is_dir() {
            return Err(WorkspaceError::NotInitialized(root));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn devbox_dir(&self) -> PathBuf {
        self.root.join(DEVBOX_DIR)
    }

    pub fn workspace_dir(&self) -> PathBuf {
        self.devbox_dir().join("workspace")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.devbox_dir().join("cache")
    }

    pub fn tools_dir(&self) -> PathBuf {
        self.devbox_dir().join("tools")
    }

    pub fn tmp_dir(&self) -> PathBuf {
        self.devbox_dir().join("tmp")
    }

    pub fn home_dir(&self) -> PathBuf {
        self.devbox_dir().join("home")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let mut dir = std::env::temp_dir();
        dir.push(format!("devbox-test-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn init_creates_structure() {
        let root = temp_dir();
        let ws = Workspace::init(&root).expect("init workspace");

        assert!(ws.devbox_dir().is_dir());
        for subdir in SUBDIRS {
            assert!(ws.devbox_dir().join(subdir).is_dir(), "missing {subdir}");
        }
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn init_is_idempotent() {
        let root = temp_dir();
        Workspace::init(&root).expect("first init");
        Workspace::init(&root).expect("second init");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn open_fails_when_not_initialized() {
        let root = temp_dir();
        assert!(matches!(
            Workspace::open(&root),
            Err(WorkspaceError::NotInitialized(_))
        ));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn path_getters_point_into_devbox() {
        let root = temp_dir();
        let ws = Workspace::init(&root).expect("init workspace");

        assert_eq!(ws.workspace_dir(), ws.devbox_dir().join("workspace"));
        assert_eq!(ws.cache_dir(), ws.devbox_dir().join("cache"));
        assert_eq!(ws.tools_dir(), ws.devbox_dir().join("tools"));
        assert_eq!(ws.tmp_dir(), ws.devbox_dir().join("tmp"));
        assert_eq!(ws.home_dir(), ws.devbox_dir().join("home"));

        fs::remove_dir_all(&root).ok();
    }
}
