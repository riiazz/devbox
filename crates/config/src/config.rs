use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub services: BTreeMap<String, Service>,
    pub tools: BTreeMap<String, ToolConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
}

/// A long-running process managed by `devbox up` (version 0.9).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Service {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    /// Whether `devbox up` should start this service. Set to `false` to keep a
    /// service defined without starting it. Defaults to enabled so existing
    /// configs keep their current behavior.
    #[serde(default = "default_enabled", skip_serializing_if = "is_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn is_enabled(enabled: &bool) -> bool {
    *enabled
}

/// A user-defined tool resolvable by `devbox install` (version 0.10).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolConfig {
    pub default_version: String,
    pub executable: String,
    /// Asset filename template, e.g. `caddy_{version}_{os}_{arch}.{ext}`.
    /// Placeholders: `{name}`, `{version}`, `{os}`, `{arch}`, `{triple}`,
    /// `{ext}`. Defaults to `{name}-{version}-{triple}.{ext}`.
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub github: GithubSource,
}

/// The GitHub repository that publishes a tool's release archives.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GithubSource {
    pub owner: String,
    pub repo: String,
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
            services: BTreeMap::new(),
            tools: BTreeMap::new(),
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

    #[test]
    fn parses_services() {
        let path = temp_file("services.toml");
        fs::write(
            &path,
            r#"
[workspace]
name = "Planning"

[environment]
DOTNET_ENVIRONMENT = "Development"

[services.api]
command = "dotnet"
args = ["run", "--project", "src/Api"]
cwd = "src"
environment = { LOG_LEVEL = "info" }

[services.redis]
command = "redis-server"
"#,
        )
        .expect("write config");

        let config = Config::load(&path).expect("load config");
        let api = config.services.get("api").expect("api service");
        assert_eq!(api.command, "dotnet");
        assert_eq!(api.args, ["run", "--project", "src/Api"]);
        assert_eq!(api.cwd.as_deref(), Some(Path::new("src")));
        assert_eq!(api.environment.get("LOG_LEVEL").map(String::as_str), Some("info"));
        assert_eq!(config.services.get("redis").expect("redis service").command, "redis-server");
        assert!(!config.services.contains_key("missing"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_services_defaults_to_empty() {
        let path = temp_file("noservices.toml");
        fs::write(&path, "[workspace]\nname = \"x\"\n").expect("write config");
        let config = Config::load(&path).expect("load config");
        assert!(config.services.is_empty());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn services_round_trip() {
        let path = temp_file("services-roundtrip.toml");
        let config = Config {
            workspace: Workspace {
                name: "Planning".into(),
            },
            environment: BTreeMap::new(),
            services: BTreeMap::from([(
                "api".into(),
                Service {
                    command: "dotnet".into(),
                    args: vec!["run".into()],
                    cwd: None,
                    environment: BTreeMap::new(),
                    enabled: true,
                },
            )]),
            tools: BTreeMap::new(),
        };
        config.save(&path).expect("save config");

        let loaded = Config::load(&path).expect("load config");
        assert_eq!(loaded.services["api"].command, "dotnet");
        assert_eq!(loaded.services["api"].args, ["run"]);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn service_enabled_defaults_to_true() {
        let path = temp_file("enabled-default.toml");
        fs::write(
            &path,
            r#"
[services.api]
command = "dotnet"
"#,
        )
        .expect("write config");

        let config = Config::load(&path).expect("load config");
        assert!(config.services["api"].enabled);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn service_disabled_round_trips() {
        let path = temp_file("enabled-toggle.toml");
        fs::write(
            &path,
            r#"
[services.api]
command = "dotnet"
enabled = false
"#,
        )
        .expect("write config");

        let mut config = Config::load(&path).expect("load config");
        assert!(!config.services["api"].enabled);

        config.services.get_mut("api").expect("api service").enabled = true;
        config.save(&path).expect("save config");
        let loaded = Config::load(&path).expect("load config");
        assert!(loaded.services["api"].enabled);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn parses_tools() {
        let path = temp_file("tools.toml");
        fs::write(
            &path,
            r#"
[workspace]
name = "Planning"

[tools.git]
default_version = "2.45.0"
executable = "git"
asset = "git-{version}-{triple}.{ext}"

[tools.git.github]
owner = "git-for-windows"
repo = "git"

[tools.caddy]
default_version = "2.11.3"
executable = "caddy"
asset = "caddy_{version}_{os}_{arch}.{ext}"

[tools.caddy.github]
owner = "caddyserver"
repo = "caddy"

[tools.rg]
default_version = "14.1.0"
executable = "rg"

[tools.rg.github]
owner = "BurntSushi"
repo = "ripgrep"
"#,
        )
        .expect("write config");

        let config = Config::load(&path).expect("load config");
        let git = config.tools.get("git").expect("git tool");
        assert_eq!(git.default_version, "2.45.0");
        assert_eq!(git.executable, "git");
        assert_eq!(git.github.owner, "git-for-windows");
        assert_eq!(git.github.repo, "git");
        assert_eq!(git.asset.as_deref(), Some("git-{version}-{triple}.{ext}"));
        let caddy = config.tools.get("caddy").expect("caddy tool");
        assert_eq!(caddy.asset.as_deref(), Some("caddy_{version}_{os}_{arch}.{ext}"));
        assert_eq!(config.tools["rg"].executable, "rg");
        assert_eq!(config.tools["rg"].asset, None);
        assert!(!config.tools.contains_key("missing"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_tools_defaults_to_empty() {
        let path = temp_file("notools.toml");
        fs::write(&path, "[workspace]\nname = \"x\"\n").expect("write config");
        let config = Config::load(&path).expect("load config");
        assert!(config.tools.is_empty());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn tools_round_trip() {
        let path = temp_file("tools-roundtrip.toml");
        let config = Config {
            workspace: Workspace {
                name: "Planning".into(),
            },
            environment: BTreeMap::new(),
            services: BTreeMap::new(),
            tools: BTreeMap::from([(
                "git".into(),
                ToolConfig {
                    default_version: "2.45.0".into(),
                    executable: "git".into(),
                    asset: None,
                    github: GithubSource {
                        owner: "git-for-windows".into(),
                        repo: "git".into(),
                    },
                },
            )]),
        };
        config.save(&path).expect("save config");

        let loaded = Config::load(&path).expect("load config");
        assert_eq!(loaded.tools["git"].github.repo, "git");

        fs::remove_file(&path).ok();
    }
}
