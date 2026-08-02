use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, IsTerminal, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::{Process, SupervisorError};

/// Options controlling the live `devbox up` dashboard.
pub struct Options {
    /// Services to show logs for. Empty means the first five services.
    pub watch: Vec<String>,
    /// Number of trailing log lines shown per service.
    pub log_lines: usize,
    /// How often the dashboard refreshes.
    pub refresh: Duration,
    /// Raised externally (e.g. a Ctrl+C handler) to request graceful shutdown.
    pub stop: Arc<AtomicBool>,
}

/// How the dashboard loop ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Every service exited on its own.
    AllExited,
    /// The stop flag was raised (e.g. Ctrl+C).
    Interrupted,
}

/// A live sample of a process's cumulative CPU time and memory.
#[derive(Debug, Clone)]
pub struct ProcessSample {
    /// Cumulative CPU time in nanoseconds.
    cpu_time_ns: u128,
    /// Working set in bytes.
    memory_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct Metrics {
    cpu_percent: Option<f64>,
    memory_bytes: Option<u64>,
}

/// Runs the live dashboard until every service exits or the stop flag is
/// raised. The screen is redrawn once per `refresh` interval.
pub fn run(children: &mut [(Process, Child)], opts: &Options) -> Result<Exit, SupervisorError> {
    let log_order: Vec<String> = if opts.watch.is_empty() {
        children.iter().take(5).map(|(p, _)| p.name.clone()).collect()
    } else {
        opts.watch.to_vec()
    };
    let mut tails: Vec<(String, LogTail)> = log_order
        .into_iter()
        .map(|name| (name, LogTail::new(opts.log_lines.max(1))))
        .collect();

    let mut prev_samples: HashMap<u32, (ProcessSample, Instant)> = HashMap::new();
    let mut exited: HashMap<String, i32> = HashMap::new();

    loop {
        if opts.stop.load(Ordering::SeqCst) {
            println!("devbox: received interrupt, stopping services");
            return Ok(Exit::Interrupted);
        }

        for (process, child) in children.iter_mut() {
            if exited.contains_key(&process.name) {
                continue;
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    exited.insert(process.name.clone(), status.code().unwrap_or(-1));
                }
                Ok(None) => {}
                Err(source) => {
                    return Err(SupervisorError::Wait {
                        name: process.name.clone(),
                        source,
                    });
                }
            }
        }

        for (process, _) in children.iter() {
            if let Some((_, tail)) = tails.iter_mut().find(|(name, _)| *name == process.name) {
                tail.update(&process.log_file);
            }
        }

        let running_pids: Vec<u32> = children
            .iter()
            .filter(|(p, _)| !exited.contains_key(&p.name))
            .map(|(p, _)| p.pid)
            .collect();
        let samples = sample_processes(&running_pids);
        let listening = listening_endpoints(&running_pids);

        let mut metrics: HashMap<u32, Metrics> = HashMap::new();
        for (process, _) in children.iter() {
            if exited.contains_key(&process.name) {
                continue;
            }
            let m = match samples.get(&process.pid) {
                Some(cur) => {
                    let cpu_percent = match prev_samples.get(&process.pid) {
                        Some((prev, at)) if cur.cpu_time_ns >= prev.cpu_time_ns => {
                            let wall = at.elapsed();
                            if wall.as_nanos() == 0 {
                                None
                            } else {
                                let cores = std::thread::available_parallelism()
                                    .map(|n| n.get())
                                    .unwrap_or(1) as f64;
                                Some(
                                    (cur.cpu_time_ns - prev.cpu_time_ns) as f64
                                        / wall.as_nanos() as f64
                                        / cores
                                        * 100.0,
                                )
                            }
                        }
                        _ => None,
                    };
                    Metrics {
                        cpu_percent,
                        memory_bytes: Some(cur.memory_bytes),
                    }
                }
                None => Metrics {
                    cpu_percent: None,
                    memory_bytes: None,
                },
            };
            metrics.insert(process.pid, m);
        }
        for (pid, sample) in &samples {
            prev_samples.insert(*pid, (sample.clone(), Instant::now()));
        }

        render(children, &exited, &metrics, &listening, &tails);

        let all_done = children.iter().all(|(p, _)| exited.contains_key(&p.name));
        if all_done {
            return Ok(Exit::AllExited);
        }
        std::thread::sleep(opts.refresh);
    }
}

fn render(
    children: &[(Process, Child)],
    exited: &HashMap<String, i32>,
    metrics: &HashMap<u32, Metrics>,
    listening: &HashMap<u32, Vec<String>>,
    tails: &[(String, LogTail)],
) {
    if io::stdout().is_terminal() {
        clear_screen();
    } else {
        println!();
        println!("--- devbox dashboard ---");
    }

    println!("devbox running with pid: {}", std::process::id());
    println!();

    let headers: Vec<String> = [
        "service",
        "status",
        "pid",
        "parent_pid",
        "cpu",
        "memory",
        "listening",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (process, _) in children {
        let status = match exited.get(&process.name) {
            Some(code) => format!("exited ({code})"),
            None => "running".to_string(),
        };
        let m = metrics.get(&process.pid);
        let cpu = m
            .and_then(|m| m.cpu_percent)
            .map(|v| format!("{v:.0}%"))
            .unwrap_or_default();
        let mem = m
            .and_then(|m| m.memory_bytes)
            .map(format_bytes)
            .unwrap_or_default();
        let listen = listening.get(&process.pid).map(|e| e.join(" ")).unwrap_or_default();
        rows.push(vec![
            process.name.clone(),
            status,
            process.pid.to_string(),
            process.parent_pid.to_string(),
            cpu,
            mem,
            listen,
        ]);
    }

    let widths = column_widths(&headers, &rows);
    let right = [false, false, true, true, true, true, false];
    println!("{}", format_row(&headers, &widths, &right));
    println!("{}", format_separator(&widths));
    for row in &rows {
        println!("{}", format_row(row, &widths, &right));
    }

    if !tails.is_empty() {
        println!();
        println!("logs:");
        for (name, tail) in tails {
            println!("---");
            println!("service: {name}");
            for line in tail.render() {
                println!("{line}");
            }
        }
    }

    let _ = io::stdout().flush();
}

fn column_widths(headers: &[String], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    widths
}

fn format_row(cells: &[String], widths: &[usize], right: &[bool]) -> String {
    let mut s = String::from("|");
    for (i, cell) in cells.iter().enumerate() {
        let width = widths.get(i).copied().unwrap_or(cell.len());
        if right.get(i).copied().unwrap_or(false) {
            s.push_str(&format!(" {cell:>width$} |"));
        } else {
            s.push_str(&format!(" {cell:<width$} |"));
        }
    }
    s
}

fn format_separator(widths: &[usize]) -> String {
    let mut s = String::from("|");
    for width in widths {
        s.push_str(&format!(" {} |", "-".repeat(*width)));
    }
    s
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1}gb", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}mb", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0}kb", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}b")
    }
}

fn format_endpoint(endpoint: &str) -> String {
    match endpoint.rsplit_once(':') {
        Some((addr, port)) => {
            let host = match addr {
                "127.0.0.1" | "::1" => "localhost",
                "0.0.0.0" | "::" | "*" => "*",
                other => other,
            };
            format!("{host}:{port}")
        }
        None => endpoint.to_string(),
    }
}

#[cfg(windows)]
fn sample_processes(pids: &[u32]) -> HashMap<u32, ProcessSample> {
    let mut out = HashMap::new();
    if pids.is_empty() {
        return out;
    }
    let filter = pids
        .iter()
        .map(|p| format!("ProcessId = {p}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let script = format!(
        "Get-CimInstance Win32_Process -Filter '{}' | ForEach-Object {{ \"$($_.ProcessId),$($_.KernelModeTime),$($_.UserModeTime),$($_.WorkingSetSize)\" }}",
        filter
    );
    for line in run_powershell(&script).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split(',');
        let parsed = parts.next().and_then(|s| s.trim().parse::<u32>().ok());
        let kernel = parts.next().and_then(|s| s.trim().parse::<u64>().ok());
        let user = parts.next().and_then(|s| s.trim().parse::<u64>().ok());
        let mem = parts.next().and_then(|s| s.trim().parse::<u64>().ok());
        if let (Some(pid), Some(kernel), Some(user), Some(mem)) = (parsed, kernel, user, mem) {
            out.insert(
                pid,
                ProcessSample {
                    cpu_time_ns: u128::from(kernel.saturating_add(user)).saturating_mul(100),
                    memory_bytes: mem,
                },
            );
        }
    }
    out
}

#[cfg(unix)]
fn sample_processes(pids: &[u32]) -> HashMap<u32, ProcessSample> {
    let mut out = HashMap::new();
    if pids.is_empty() {
        return out;
    }
    let list = pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
    let Ok(output) = Command::new("ps")
        .args(["-o", "pid=,time=,rss=", "-p", &list])
        .output()
    else {
        return out;
    };
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut fields = line.split_whitespace();
        let Some(pid) = fields.next().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Some(time) = fields.next() else {
            continue;
        };
        let Some(rss_kb) = fields.next().and_then(|s| s.parse::<u64>().ok()) else {
            continue;
        };
        let Some(cpu_secs) = parse_cpu_time(time) else {
            continue;
        };
        out.insert(
            pid,
            ProcessSample {
                cpu_time_ns: cpu_secs * 1_000_000_000,
                memory_bytes: rss_kb * 1024,
            },
        );
    }
    out
}

#[cfg(unix)]
fn parse_cpu_time(s: &str) -> Option<u64> {
    let parts: Vec<u64> = s.split(':').map(|p| p.parse().ok()).collect::<Option<Vec<u64>>>()?;
    match parts.as_slice() {
        [min, sec] => Some(min * 60 + sec),
        [hr, min, sec] => Some(hr * 3600 + min * 60 + sec),
        _ => None,
    }
}

#[cfg(windows)]
fn listening_endpoints(pids: &[u32]) -> HashMap<u32, Vec<String>> {
    let mut out: HashMap<u32, Vec<String>> = HashMap::new();
    if pids.is_empty() {
        return out;
    }
    let list = pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
    let script = format!(
        "Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object {{ $_.OwningProcess -in @({list}) }} | ForEach-Object {{ \"$($_.OwningProcess),$($_.LocalAddress):$($_.LocalPort)\" }}"
    );
    for line in run_powershell(&script).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((pid, endpoint)) = line.split_once(',') else {
            continue;
        };
        let Ok(pid) = pid.trim().parse::<u32>() else {
            continue;
        };
        out.entry(pid).or_default().push(format_endpoint(endpoint));
    }
    out
}

#[cfg(unix)]
fn listening_endpoints(pids: &[u32]) -> HashMap<u32, Vec<String>> {
    let mut out: HashMap<u32, Vec<String>> = HashMap::new();
    if pids.is_empty() {
        return out;
    }
    let Ok(output) = Command::new("ss").args(["-H", "-tlnp"]).output() else {
        return out;
    };
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some(pid) = pids.iter().copied().find(|p| line.contains(&format!("pid={p},"))) else {
            continue;
        };
        let cols: Vec<&str> = line.split_whitespace().collect();
        if let Some(local) = cols.get(3) {
            out.entry(pid).or_default().push(format_endpoint(local));
        }
    }
    out
}

#[cfg(windows)]
fn run_powershell(script: &str) -> String {
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn clear_screen() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        #[cfg(windows)]
        enable_ansi();
    });
    print!("\x1b[2J\x1b[H");
}

#[cfg(windows)]
fn enable_ansi() {
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_OUTPUT_HANDLE,
    };
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle == -1 {
            return;
        }
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return;
        }
        let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
    }
}

/// Incrementally follows a service log file, keeping only the most recent
/// `max_lines` lines in memory.
struct LogTail {
    max_lines: usize,
    pos: u64,
    lines: VecDeque<String>,
}

impl LogTail {
    fn new(max_lines: usize) -> Self {
        Self {
            max_lines,
            pos: 0,
            lines: VecDeque::new(),
        }
    }

    fn update(&mut self, path: &Path) {
        let Ok(mut file) = File::open(path) else {
            return;
        };
        let Ok(len) = file.metadata().map(|m| m.len()) else {
            return;
        };
        if len < self.pos {
            self.pos = 0;
            self.lines.clear();
        }
        if len == self.pos {
            return;
        }
        if file.seek(SeekFrom::Start(self.pos)).is_err() {
            return;
        }
        let mut buf = Vec::new();
        if file.read_to_end(&mut buf).is_err() {
            return;
        }
        self.pos = len;
        for line in String::from_utf8_lossy(&buf).split('\n') {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            self.lines.push_back(line.to_string());
            if self.lines.len() > self.max_lines {
                self.lines.pop_front();
            }
        }
    }

    fn render(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::Supervisor;
    use config::Service;
    use runtime::Environment;

    fn temp_dir() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("devbox-dash-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn quick_service() -> Service {
        if cfg!(windows) {
            Service {
                command: "cmd".into(),
                args: vec!["/C".into(), "echo hello".into()],
                cwd: None,
                environment: BTreeMap::new(),
            }
        } else {
            Service {
                command: "sh".into(),
                args: vec!["-c".into(), "echo hello".into()],
                cwd: None,
                environment: BTreeMap::new(),
            }
        }
    }

    #[test]
    fn run_returns_all_exited_when_children_finish() {
        let base = temp_dir();
        let sup = Supervisor::new(base.join("state.toml"), base.join("logs"), &base);
        let env = Environment::from_current();
        let services = BTreeMap::from([("echoer".to_string(), quick_service())]);

        let mut spawned = sup.spawn_all(&services, &env).expect("spawn");
        let opts = Options {
            watch: Vec::new(),
            log_lines: 5,
            refresh: Duration::from_millis(10),
            stop: Arc::new(AtomicBool::new(false)),
        };
        let exit = run(&mut spawned, &opts).expect("dashboard");
        assert_eq!(exit, Exit::AllExited);

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn run_returns_interrupted_when_stop_flag_set() {
        let base = temp_dir();
        let sup = Supervisor::new(base.join("state.toml"), base.join("logs"), &base);
        let env = Environment::from_current();
        let services = BTreeMap::from([("sleeper".to_string(), sleeper_service())]);

        let mut spawned = sup.spawn_all(&services, &env).expect("spawn");
        let opts = Options {
            watch: Vec::new(),
            log_lines: 5,
            refresh: Duration::from_millis(10),
            stop: Arc::new(AtomicBool::new(true)),
        };
        let exit = run(&mut spawned, &opts).expect("dashboard");
        assert_eq!(exit, Exit::Interrupted);

        sup.stop(None).ok();
        fs::remove_dir_all(&base).ok();
    }

    fn sleeper_service() -> Service {
        if cfg!(windows) {
            Service {
                command: "cmd".into(),
                args: vec!["/C".into(), "ping -n 60 127.0.0.1 > nul".into()],
                cwd: None,
                environment: BTreeMap::new(),
            }
        } else {
            Service {
                command: "sleep".into(),
                args: vec!["60".into()],
                cwd: None,
                environment: BTreeMap::new(),
            }
        }
    }

    #[test]
    fn log_tail_keeps_last_lines() {
        let base = temp_dir();
        let path = base.join("svc.log");
        fs::write(&path, "a\nb\nc\nd\ne\n").expect("write log");

        let mut tail = LogTail::new(2);
        tail.update(&path);
        let lines: Vec<&str> = tail.render().collect();
        assert_eq!(lines, vec!["d", "e"]);

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn format_bytes_human_readable() {
        assert_eq!(format_bytes(0), "0b");
        assert_eq!(format_bytes(2048), "2kb");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0mb");
        assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0gb");
    }

    #[test]
    fn format_endpoint_maps_loopback() {
        assert_eq!(format_endpoint("127.0.0.1:2009"), "localhost:2009");
        assert_eq!(format_endpoint("0.0.0.0:80"), "*:80");
        assert_eq!(format_endpoint("10.0.0.5:4041"), "10.0.0.5:4041");
    }
}
