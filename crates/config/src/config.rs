use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FILE_NAME: &str = "devbox.toml";

#[derive(Debug, Error)]
pub enum ConfigError {
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub workspace: Workspace,
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&contents).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let contents =
            toml::to_string_pretty(self).map_err(|source| ConfigError::Serialize {
                path: path.to_path_buf(),
                source,
            })?;
        fs::write(path, contents).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("devbox-config-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir.join(name)
    }

    #[test]
    fn parses_workspace_and_environment() {
        let path = temp_file("full.toml");
        fs::write(
            &path,
            r#"
[workspace]
name = "Planning"

[environment]
DOTNET_ENVIRONMENT = "Development"
"#,
        )
        .expect("write config");

        let config = Config::load(&path).expect("load config");
        assert_eq!(config.workspace.name, "Planning");
        assert_eq!(
            config.environment.get("DOTNET_ENVIRONMENT").map(String::as_str),
            Some("Development")
        );

        fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_fields_default() {
        let path = temp_file("empty.toml");
        fs::write(&path, "").expect("write config");

        let config = Config::load(&path).expect("load config");
        assert_eq!(config.workspace.name, "");
        assert!(config.environment.is_empty());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn save_round_trips() {
        let path = temp_file("roundtrip.toml");
        let config = Config {
            workspace: Workspace {
                name: "Planning".into(),
            },
            environment: BTreeMap::from([("A".into(), "B".into())]),
        };
        config.save(&path).expect("save config");

        let loaded = Config::load(&path).expect("load config");
        assert_eq!(loaded.workspace.name, "Planning");
        assert_eq!(loaded.environment.get("A").map(String::as_str), Some("B"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_invalid_toml() {
        let path = temp_file("invalid.toml");
        fs::write(&path, "not toml [").expect("write config");
        assert!(matches!(Config::load(&path), Err(ConfigError::Parse { .. })));
        fs::remove_file(&path).ok();
    }
}
