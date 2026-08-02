use std::path::{Path, PathBuf};

/// The platform executable name for a tool, e.g. `rg` -> `rg.exe` on Windows.
pub fn executable_name(name: &str) -> String {
    format!("{name}{}", if cfg!(windows) { ".exe" } else { "" })
}

/// Recursively locates `file_name` under `dir`.
pub fn find_file(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, file_name) {
                return Some(found);
            }
        } else if path
            .file_name()
            .map(|name| name.to_string_lossy() == file_name)
            .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}
