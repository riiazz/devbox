use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Name of the supervisor state file inside `.devbox/workspace/`.
pub const STATE_FILE: &str = "processes.toml";

#[derive(Debug, Error)]
pub enum StateError {
    #[error("failed to read state `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse state `{path}`: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to write state `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to serialize state `{path}`: {source}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
}

/// A process recorded in the supervisor state file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Process {
    pub name: String,
    pub pid: u32,
    pub log_file: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StateFile {
    #[serde(default)]
    processes: Vec<Process>,
}

/// Reads and writes the supervisor state file.
#[derive(Debug)]
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Vec<Process>, StateError> {
        if !self.path.is_file() {
            return Ok(Vec::new());
        }
        let contents = fs::read_to_string(&self.path).map_err(|source| StateError::Read {
            path: self.path.clone(),
            source,
        })?;
        let file: StateFile = toml::from_str(&contents).map_err(|source| StateError::Parse {
            path: self.path.clone(),
            source,
        })?;
        Ok(file.processes)
    }

    pub fn save(&self, processes: &[Process]) -> Result<(), StateError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| StateError::Write {
                path: self.path.clone(),
                source,
            })?;
        }
        let file = StateFile {
            processes: processes.to_vec(),
        };
        let contents = toml::to_string_pretty(&file).map_err(|source| StateError::Serialize {
            path: self.path.clone(),
            source,
        })?;
        fs::write(&self.path, contents).map_err(|source| StateError::Write {
            path: self.path.clone(),
            source,
        })
    }

    /// Removes the state file. Missing file is not an error.
    pub fn clear(&self) -> Result<(), StateError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(StateError::Write {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn temp_path() -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("devbox-state-{}-{}", std::process::id(), n))
    }

    fn sample(name: &str, pid: u32) -> Process {
        Process {
            name: name.into(),
            pid,
            log_file: PathBuf::from(".devbox").join("workspace").join("logs").join(format!("{name}.log")),
        }
    }

    #[test]
    fn load_missing_is_empty() {
        let store = StateStore::new(temp_path());
        assert!(store.load().expect("load").is_empty());
    }

    #[test]
    fn save_load_round_trips() {
        let path = temp_path();
        let store = StateStore::new(&path);
        store.save(&[sample("api", 1234), sample("redis", 5678)]).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded, vec![sample("api", 1234), sample("redis", 5678)]);
        fs::remove_file(&path).ok();
    }

    #[test]
    fn clear_removes_file() {
        let path = temp_path();
        let store = StateStore::new(&path);
        store.save(&[sample("api", 1234)]).expect("save");
        store.clear().expect("clear");
        assert!(!path.is_file());
        assert!(store.load().expect("load").is_empty());
    }

    #[test]
    fn clear_missing_is_ok() {
        let store = StateStore::new(temp_path());
        assert!(store.clear().is_ok());
    }

    #[test]
    fn load_rejects_invalid_toml() {
        let path = temp_path();
        fs::write(&path, "not toml [").expect("write state");
        let store = StateStore::new(&path);
        assert!(matches!(store.load(), Err(StateError::Parse { .. })));
        fs::remove_file(&path).ok();
    }
}
