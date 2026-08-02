use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::tool::{Tool, ToolKey};

/// Registry manifest stored inside `.devbox/tools/`.
pub const REGISTRY_FILE: &str = "registry.toml";

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("failed to read `{path}`: {source}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse `{path}`: {source}")]
    Parse {
        path: std::path::PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to write `{path}`: {source}")]
    Write {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize `{path}`: {source}")]
    Serialize {
        path: std::path::PathBuf,
        #[source]
        source: toml::ser::Error,
    },
}

/// The on-disk shape of a tool registry.
#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryFile {
    #[serde(default)]
    tools: Vec<Tool>,
}

/// An ordered collection of registered tools.
///
/// Tools are keyed by `(name, version)` so a single tool can be registered at
/// multiple versions. Version 0.4 holds metadata only; there are no downloads.
#[derive(Debug, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<ToolKey, Tool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a tool, replacing any tool with the same name and version.
    pub fn register(&mut self, tool: Tool) {
        self.tools.insert(tool.key(), tool);
    }

    /// Removes every version of the named tool.
    /// Returns the number of versions removed.
    pub fn unregister(&mut self, name: &str) -> usize {
        let keys: Vec<ToolKey> = self
            .tools
            .keys()
            .filter(|key| key.name == name)
            .cloned()
            .collect();
        let count = keys.len();
        for key in keys {
            self.tools.remove(&key);
        }
        count
    }

    /// Removes a single name/version pair.
    pub fn unregister_version(&mut self, name: &str, version: &str) -> bool {
        self.tools
            .remove(&ToolKey {
                name: name.to_string(),
                version: version.to_string(),
            })
            .is_some()
    }

    /// Returns the highest registered version of the named tool.
    pub fn get(&self, name: &str) -> Option<&Tool> {
        self.tools
            .iter()
            .filter(|(key, _)| key.name == name)
            .map(|(_, tool)| tool)
            .max_by(|a, b| a.version.cmp(&b.version))
    }

    /// Returns a specific name/version pair.
    pub fn get_exact(&self, name: &str, version: &str) -> Option<&Tool> {
        self.tools
            .get(&ToolKey {
                name: name.to_string(),
                version: version.to_string(),
            })
    }

    /// Returns every registered version of the named tool, ascending by version.
    pub fn versions(&self, name: &str) -> Vec<&Tool> {
        self.tools
            .iter()
            .filter(|(key, _)| key.name == name)
            .map(|(_, tool)| tool)
            .collect()
    }

    /// Returns all registered tools, ordered by name then version.
    pub fn list(&self) -> Vec<&Tool> {
        self.tools.values().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn load(path: &Path) -> Result<Self, RegistryError> {
        let contents = fs::read_to_string(path).map_err(|source| RegistryError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let file: RegistryFile =
            toml::from_str(&contents).map_err(|source| RegistryError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
        let mut registry = Self::default();
        for tool in file.tools {
            registry.register(tool);
        }
        Ok(registry)
    }

    pub fn save(&self, path: &Path) -> Result<(), RegistryError> {
        let file = RegistryFile {
            tools: self.tools.values().cloned().collect(),
        };
        let contents =
            toml::to_string_pretty(&file).map_err(|source| RegistryError::Serialize {
                path: path.to_path_buf(),
                source,
            })?;
        fs::write(path, contents).map_err(|source| RegistryError::Write {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ripgrep(version: &str) -> Tool {
        Tool::new(
            "ripgrep",
            version,
            "rg",
            Path::new(".devbox").join("tools").join("rg").join(version),
        )
    }

    fn temp_file(name: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("devbox-toolchain-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir.join(name)
    }

    #[test]
    fn register_upserts_same_name_and_version() {
        let mut registry = ToolRegistry::new();
        registry.register(ripgrep("14.1.0"));
        registry.register(Tool::new(
            "ripgrep",
            "14.1.0",
            "rg-new",
            Path::new("new-dir"),
        ));
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get("ripgrep").unwrap().executable, "rg-new");
    }

    #[test]
    fn get_returns_highest_version() {
        let mut registry = ToolRegistry::new();
        registry.register(ripgrep("13.0.0"));
        registry.register(ripgrep("14.1.0"));
        assert_eq!(registry.get("ripgrep").unwrap().version, "14.1.0");
    }

    #[test]
    fn get_exact_finds_specific_version() {
        let mut registry = ToolRegistry::new();
        registry.register(ripgrep("13.0.0"));
        registry.register(ripgrep("14.1.0"));
        assert!(registry.get_exact("ripgrep", "13.0.0").is_some());
        assert!(registry.get_exact("ripgrep", "9.9.9").is_none());
    }

    #[test]
    fn versions_returns_all_matching() {
        let mut registry = ToolRegistry::new();
        registry.register(ripgrep("13.0.0"));
        registry.register(ripgrep("14.1.0"));
        registry.register(Tool::new("dotnet", "8.0", "dotnet", Path::new("x")));
        assert_eq!(registry.versions("ripgrep").len(), 2);
        assert_eq!(registry.versions("missing").len(), 0);
    }

    #[test]
    fn unregister_removes_all_versions() {
        let mut registry = ToolRegistry::new();
        registry.register(ripgrep("13.0.0"));
        registry.register(ripgrep("14.1.0"));
        assert_eq!(registry.unregister("ripgrep"), 2);
        assert!(registry.is_empty());
    }

    #[test]
    fn unregister_version_removes_one() {
        let mut registry = ToolRegistry::new();
        registry.register(ripgrep("13.0.0"));
        registry.register(ripgrep("14.1.0"));
        assert!(registry.unregister_version("ripgrep", "13.0.0"));
        assert!(!registry.unregister_version("ripgrep", "13.0.0"));
        assert_eq!(registry.get("ripgrep").unwrap().version, "14.1.0");
    }

    #[test]
    fn list_is_ordered_by_name_then_version() {
        let mut registry = ToolRegistry::new();
        registry.register(ripgrep("14.1.0"));
        registry.register(Tool::new("dotnet", "8.0", "dotnet", Path::new("x")));
        let names: Vec<&str> = registry.list().iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["dotnet", "ripgrep"]);
    }

    #[test]
    fn save_load_round_trips() {
        let path = temp_file("roundtrip.toml");
        let mut registry = ToolRegistry::new();
        registry.register(ripgrep("14.1.0"));

        registry.save(&path).expect("save registry");
        let loaded = ToolRegistry::load(&path).expect("load registry");
        let tool = loaded.get("ripgrep").expect("tool present");
        assert_eq!(tool.version, "14.1.0");
        assert_eq!(tool.executable, "rg");
        assert_eq!(
            tool.install_dir,
            Path::new(".devbox").join("tools").join("rg").join("14.1.0")
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn load_empty_file_is_empty_registry() {
        let path = temp_file("empty.toml");
        fs::write(&path, "").expect("write empty registry");
        let registry = ToolRegistry::load(&path).expect("load empty registry");
        assert!(registry.is_empty());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn load_rejects_invalid_toml() {
        let path = temp_file("invalid.toml");
        fs::write(&path, "not toml [").expect("write invalid registry");
        assert!(matches!(ToolRegistry::load(&path), Err(RegistryError::Parse { .. })));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn load_missing_file_is_error() {
        let path = temp_file("missing.toml");
        assert!(matches!(ToolRegistry::load(&path), Err(RegistryError::Read { .. })));
    }
}
