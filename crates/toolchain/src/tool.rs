use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A tool installed into the workspace.
///
/// Version 0.4 introduces the registry only; nothing is downloaded yet.
/// Installed tool archives and extracted installs arrive in version 0.5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub version: String,
    pub executable: String,
    pub install_dir: PathBuf,
}

impl Tool {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        executable: impl Into<String>,
        install_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            executable: executable.into(),
            install_dir: install_dir.into(),
        }
    }

    pub fn key(&self) -> ToolKey {
        ToolKey {
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }
}

/// Uniquely identifies a tool by name and version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ToolKey {
    pub name: String,
    pub version: String,
}
