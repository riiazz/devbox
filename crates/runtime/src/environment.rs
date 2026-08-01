use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

const PATH_KEY: &str = "PATH";

#[derive(Debug, Clone)]
pub struct Environment {
    vars: HashMap<String, String>,
    path: Vec<PathBuf>,
}

impl Environment {
    pub fn from_current() -> Self {
        let mut vars = HashMap::new();
        for (key, value) in std::env::vars() {
            if key.eq_ignore_ascii_case(PATH_KEY) {
                continue;
            }
            vars.insert(key, value);
        }
        let path = std::env::var_os(PATH_KEY)
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        Self { vars, path }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        if key.eq_ignore_ascii_case(PATH_KEY) {
            return std::env::join_paths(&self.path)
                .ok()
                .map(|p| p.to_string_lossy().into_owned());
        }
        self.vars.get(key).cloned()
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if key.eq_ignore_ascii_case(PATH_KEY) {
            self.path = std::env::split_paths(&value).collect();
            return;
        }
        self.vars.insert(key, value);
    }

    pub fn remove(&mut self, key: &str) {
        if key.eq_ignore_ascii_case(PATH_KEY) {
            self.path.clear();
            return;
        }
        self.vars.remove(key);
    }

    pub fn path(&self) -> &[PathBuf] {
        &self.path
    }

    pub fn prepend_path(&mut self, dir: impl Into<PathBuf>) {
        self.path.insert(0, dir.into());
    }

    pub fn append_path(&mut self, dir: impl Into<PathBuf>) {
        self.path.push(dir.into());
    }

    pub fn apply(&self, cmd: &mut Command) {
        cmd.env_clear();
        for (key, value) in &self.vars {
            cmd.env(key, value);
        }
        if let Ok(path) = std::env::join_paths(&self.path) {
            cmd.env(PATH_KEY, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_current_contains_existing_var() {
        let env = Environment::from_current();
        assert!(env.get("PATH").is_some());
    }

    #[test]
    fn set_and_get() {
        let mut env = Environment::from_current();
        env.set("DEVBOX_TEST", "value");
        assert_eq!(env.get("DEVBOX_TEST").as_deref(), Some("value"));
    }

    #[test]
    fn prepend_path_puts_dir_first() {
        let mut env = Environment::from_current();
        env.prepend_path("/devbox/tools/bin");
        assert_eq!(env.path().first().unwrap(), &PathBuf::from("/devbox/tools/bin"));
    }

    #[test]
    fn apply_writes_environment() {
        let mut env = Environment::from_current();
        env.set("DEVBOX_TEST", "value");
        let mut cmd = Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        if cfg!(windows) {
            cmd.args(["/C", "echo %DEVBOX_TEST%"]);
        } else {
            cmd.args(["-c", "echo $DEVBOX_TEST"]);
        }
        env.apply(&mut cmd);
        let out = cmd.output().expect("run child");
        assert!(String::from_utf8_lossy(&out.stdout).contains("value"));
    }
}
