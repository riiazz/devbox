use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use config::Service;
use runtime::Environment;
use thiserror::Error;

use crate::state::{Process, StateStore};

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("failed to spawn `{name}`: {source}")]
    Spawn {
        name: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to wait on `{name}`: {source}")]
    Wait {
        name: String,
        #[source]
        source: io::Error,
    },
    #[error("failed to open log for `{name}`: {source}")]
    Log {
        name: String,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    State(#[from] crate::state::StateError),
}

/// Status of a supervised process.
#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub pid: u32,
    pub running: bool,
    pub log_file: PathBuf,
}

/// Manages the long-running services declared in `[services]`.
pub struct Supervisor {
    state: StateStore,
    log_dir: PathBuf,
    base_dir: PathBuf,
}

impl Supervisor {
    /// `state_path` is the state file, `log_dir` where per-service logs are
    /// appended, and `base_dir` resolves relative service working directories.
    pub fn new(
        state_path: impl Into<PathBuf>,
        log_dir: impl Into<PathBuf>,
        base_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            state: StateStore::new(state_path),
            log_dir: log_dir.into(),
            base_dir: base_dir.into(),
        }
    }

    /// Spawns every service, records it in the state file, and returns the
    /// live child handles. On failure all previously spawned children are
    /// killed and the state file is left untouched.
    pub fn spawn_all(
        &self,
        services: &BTreeMap<String, Service>,
        env: &Environment,
    ) -> Result<Vec<(Process, Child)>, SupervisorError> {
        let mut spawned: Vec<(Process, Child)> = Vec::new();
        for (name, service) in services {
            match self.spawn_one(name, service, env) {
                Ok(pair) => spawned.push(pair),
                Err(err) => {
                    for (_, mut child) in spawned {
                        let _ = child.kill();
                    }
                    return Err(err);
                }
            }
        }

        let processes: Vec<Process> = spawned.iter().map(|(p, _)| p.clone()).collect();
        self.state.save(&processes)?;
        Ok(spawned)
    }

    fn spawn_one(
        &self,
        name: &str,
        service: &Service,
        env: &Environment,
    ) -> Result<(Process, Child), SupervisorError> {
        fs::create_dir_all(&self.log_dir).map_err(|source| SupervisorError::Log {
            name: name.to_string(),
            source,
        })?;
        let log_file = self.log_dir.join(format!("{name}.log"));
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|source| SupervisorError::Log {
                name: name.to_string(),
                source,
            })?;
        let err_file = file.try_clone().map_err(|source| SupervisorError::Log {
            name: name.to_string(),
            source,
        })?;

        let mut cmd = Command::new(&service.command);
        cmd.args(&service.args);
        if let Some(cwd) = &service.cwd {
            let cwd = if cwd.is_absolute() {
                cwd.clone()
            } else {
                self.base_dir.join(cwd)
            };
            cmd.current_dir(&cwd);
        }
        env.apply(&mut cmd);
        for (key, value) in &service.environment {
            cmd.env(key, value);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::from(file))
            .stderr(Stdio::from(err_file));

        let child = cmd.spawn().map_err(|source| SupervisorError::Spawn {
            name: name.to_string(),
            source,
        })?;
        Ok((
            Process {
                name: name.to_string(),
                pid: child.id(),
                log_file,
            },
            child,
        ))
    }

    /// Blocks until every child has exited, printing each exit as it happens.
    pub fn monitor(&self, children: &mut [(Process, Child)]) -> Result<(), SupervisorError> {
        let mut exited = std::collections::HashSet::new();
        loop {
            let mut alive = false;
            for (process, child) in children.iter_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if exited.insert(process.name.clone()) {
                            println!("devbox: {} exited ({status})", process.name);
                        }
                    }
                    Ok(None) => alive = true,
                    Err(source) => {
                        return Err(SupervisorError::Wait {
                            name: process.name.clone(),
                            source,
                        })
                    }
                }
            }
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        Ok(())
    }

    /// Stops the recorded processes, optionally limited to `names`. Stale or
    /// already-dead processes are pruned from the state file regardless of
    /// whether the kill signal could be delivered.
    pub fn stop(&self, names: Option<&[String]>) -> Result<Vec<String>, SupervisorError> {
        let processes = self.state.load()?;
        let selected: Vec<&Process> = processes
            .iter()
            .filter(|p| names.is_none_or(|names| names.contains(&p.name)))
            .collect();

        for process in &selected {
            let _ = kill_pid(process.pid);
        }

        let killed: Vec<String> = selected.iter().map(|p| p.name.clone()).collect();
        let remaining: Vec<Process> = processes
            .iter()
            .filter(|p| !selected.contains(p))
            .cloned()
            .collect();
        if remaining.is_empty() {
            self.state.clear()?;
        } else {
            self.state.save(&remaining)?;
        }
        Ok(killed)
    }

    /// Reports liveness for every process in the state file.
    pub fn status(&self) -> Result<Vec<ServiceStatus>, SupervisorError> {
        let processes = self.state.load()?;
        Ok(processes
            .into_iter()
            .map(|p| ServiceStatus {
                running: pid_alive(p.pid),
                name: p.name,
                pid: p.pid,
                log_file: p.log_file,
            })
            .collect())
    }

    /// Log files for the named service (or every service when `name` is None).
    pub fn log_files(&self, name: Option<&str>) -> Result<Vec<(String, PathBuf)>, SupervisorError> {
        let processes = self.state.load()?;
        Ok(processes
            .iter()
            .filter(|p| name.is_none_or(|name| name == p.name))
            .map(|p| (p.name.clone(), p.log_file.clone()))
            .collect())
    }
}

/// Returns the last `lines` lines of a file as a string.
pub fn tail_file(path: &Path, lines: usize) -> Result<String, io::Error> {
    let contents = fs::read_to_string(path)?;
    let tail: Vec<&str> = contents.lines().rev().take(lines).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    Ok(tail.join("\n"))
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output();
    match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).contains(&pid.to_string()),
        Err(_) => false,
    }
}

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    let out = Command::new("ps")
        .args(["-o", "pid=", "-p", &pid.to_string()])
        .output();
    matches!(out, Ok(out) if String::from_utf8_lossy(&out.stdout).trim() == pid.to_string())
}

#[cfg(windows)]
fn kill_pid(pid: u32) -> Result<(), io::Error> {
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .map(|_| ())
}

#[cfg(unix)]
fn kill_pid(pid: u32) -> Result<(), io::Error> {
    Command::new("kill").arg(pid.to_string()).status().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("devbox-sup-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn sleeper_service() -> (String, Service) {
        if cfg!(windows) {
            (
                "sleeper".into(),
                Service {
                    command: "cmd".into(),
                    args: vec!["/C".into(), "ping -n 60 127.0.0.1 > nul".into()],
                    cwd: None,
                    environment: BTreeMap::new(),
                },
            )
        } else {
            (
                "sleeper".into(),
                Service {
                    command: "sleep".into(),
                    args: vec!["60".into()],
                    cwd: None,
                    environment: BTreeMap::new(),
                },
            )
        }
    }

    fn quick_service() -> (String, Service) {
        if cfg!(windows) {
            (
                "echoer".into(),
                Service {
                    command: "cmd".into(),
                    args: vec!["/C".into(), "echo hello".into()],
                    cwd: None,
                    environment: BTreeMap::new(),
                },
            )
        } else {
            (
                "echoer".into(),
                Service {
                    command: "sh".into(),
                    args: vec!["-c".into(), "echo hello".into()],
                    cwd: None,
                    environment: BTreeMap::new(),
                },
            )
        }
    }

    #[test]
    fn spawn_all_records_state_and_logs() {
        let base = temp_dir();
        let sup = Supervisor::new(base.join("state.toml"), base.join("logs"), &base);
        let env = Environment::from_current();
        let services = BTreeMap::from([quick_service()]);

        let spawned = sup.spawn_all(&services, &env).expect("spawn");
        let (process, _) = &spawned[0];

        let state = sup.status().expect("status");
        assert_eq!(state.len(), 1);
        assert_eq!(state[0].name, "echoer");
        assert!(state[0].log_file.is_file());
        assert_eq!(state[0].log_file, process.log_file);

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn monitor_returns_when_children_exit() {
        let base = temp_dir();
        let sup = Supervisor::new(base.join("state.toml"), base.join("logs"), &base);
        let env = Environment::from_current();
        let services = BTreeMap::from([quick_service()]);

        let mut spawned = sup.spawn_all(&services, &env).expect("spawn");
        sup.monitor(&mut spawned).expect("monitor");

        let state = sup.status().expect("status");
        assert!(state.iter().all(|s| !s.running));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn stop_kills_and_prunes_state() {
        let base = temp_dir();
        let sup = Supervisor::new(base.join("state.toml"), base.join("logs"), &base);
        let env = Environment::from_current();
        let services = BTreeMap::from([sleeper_service()]);

        let spawned = sup.spawn_all(&services, &env).expect("spawn");
        let process = &spawned[0].0;
        assert!(pid_alive(process.pid));

        let killed = sup.stop(None).expect("stop");
        assert_eq!(killed, vec!["sleeper"]);
        assert!(sup.status().expect("status").is_empty());
        assert!(!pid_alive(process.pid));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn stop_missing_names_returns_empty() {
        let base = temp_dir();
        let sup = Supervisor::new(base.join("state.toml"), base.join("logs"), &base);
        let env = Environment::from_current();
        sup.spawn_all(&BTreeMap::from([sleeper_service()]), &env)
            .expect("spawn");
        sup.stop(Some(&["nope".into()])).expect("stop unknown");
        assert_eq!(sup.status().expect("status").len(), 1);
        sup.stop(None).ok();
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn tail_file_returns_last_lines() {
        let base = temp_dir();
        let path = base.join("x.log");
        fs::write(&path, "a\nb\nc\nd\ne\n").expect("write log");
        assert_eq!(tail_file(&path, 2).expect("tail"), "d\ne");
        assert_eq!(tail_file(&path, 10).expect("tail"), "a\nb\nc\nd\ne");
        fs::remove_dir_all(&base).ok();
    }
}
