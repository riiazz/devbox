use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{self, IsTerminal, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Cell, Paragraph, Row, Table, Wrap};
use unicode_width::UnicodeWidthChar;

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

/// How often listening endpoints are re-queried. Process CPU/memory are sampled
/// every tick (they need fresh deltas), but TCP listen state changes slowly, so
/// the (more expensive) scan is throttled.
const LISTEN_SCAN_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum number of bytes a log tail reads per refresh. When a service floods
/// its log, unbounded reads stall the dashboard for a whole tick and it stops
/// appearing to update. Capping the per-tick read keeps the UI responsive; the
/// remaining bytes are picked up on subsequent refreshes.
const MAX_LOG_BYTES: u64 = 256 * 1024;

/// Maximum number of characters kept from a single log line. A service that
/// emits one enormous line (e.g. a megabyte JSON one-liner) would otherwise
/// dominate the whole panel and stall rendering; longer lines are truncated
/// with an ellipsis so the dashboard keeps flowing.
const MAX_LINE_CHARS: usize = 4 * 1024;

/// Per-service accent color, cycled by index so a service keeps the same color
/// across the table row and its log panel.
fn service_color(index: usize) -> Color {
    const PALETTE: [Color; 6] = [
        Color::Yellow,
        Color::Magenta,
        Color::Blue,
        Color::Green,
        Color::Cyan,
        Color::Red,
    ];
    PALETTE[index % PALETTE.len()]
}

fn format_uptime(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

/// The number of rows `line` occupies when word-wrapped at `width`, matching
/// ratatui's `WordWrapper` (trim=false) so the dashboard can auto-scroll a log
/// panel to the bottom. Using a naive `chars / width` estimate instead can
/// overshoot the real height, which makes `Paragraph::scroll` skip past the
/// text entirely and the panel render blank.
fn wrapped_rows(line: &Line<'_>, width: u16) -> usize {
    let mut rows = 0usize;
    for span in &line.spans {
        for raw in span.content.split('\n') {
            rows += wrapped_text_rows(raw, width);
        }
    }
    rows
}

/// Number of rows a single string occupies once word-wrapped at `width`
/// columns. Mirrors ratatui 0.29's `WordWrapper::process_input` (trim=false):
/// words fit whole onto a row, an unbreakable word wider than the row is split
/// across rows, and trailing whitespace that fits the leftover of a row is
/// dropped.
fn wrapped_text_rows(text: &str, width: u16) -> usize {
    let width = width.max(1) as usize;
    if text.is_empty() {
        return 0;
    }
    let mut rows = 0usize;
    let mut line: Vec<char> = Vec::new();
    let mut line_width = 0usize;
    let mut word_width = 0usize;
    let mut ws_width = 0usize;
    let mut word: Vec<char> = Vec::new();
    let mut ws: Vec<char> = Vec::new();
    let mut prev_non_ws = false;

    for ch in text.chars() {
        let is_ws = ch.is_whitespace();
        let cw = ch.width().unwrap_or(0);
        if cw > width {
            continue;
        }
        let word_found = prev_non_ws && is_ws;
        let untrimmed_overflow =
            line.is_empty() && word_width + ws_width + cw > width;
        if word_found || untrimmed_overflow {
            line.append(&mut ws);
            line_width += ws_width;
            line.append(&mut word);
            line_width += word_width;
            ws_width = 0;
            word_width = 0;
        }
        let line_full = line_width >= width;
        let word_overflow = cw > 0 && line_width + ws_width + word_width >= width;
        if line_full || word_overflow {
            let mut remaining = width.saturating_sub(line_width);
            rows += 1;
            line.clear();
            line_width = 0;
            while let Some(&c) = ws.first() {
                let ww = c.width().unwrap_or(0);
                if ww > remaining {
                    break;
                }
                ws_width -= ww;
                remaining -= ww;
                ws.remove(0);
            }
            if is_ws && ws.is_empty() {
                continue;
            }
        }
        if is_ws {
            ws_width += cw;
            ws.push(ch);
        } else {
            word_width += cw;
            word.push(ch);
        }
        prev_non_ws = !is_ws;
    }
    if !line.is_empty() || !word.is_empty() || !ws.is_empty() {
        rows += 1;
    }
    rows
}

/// Runs the live dashboard until every service exits or the stop flag is
/// raised. When stdout is a terminal the screen is redrawn in place once per
/// `refresh` interval via ratatui; otherwise a plain, appended summary is
/// printed so output stays useful when piped or captured.
pub fn run(children: &mut [(Process, Child)], opts: &Options) -> Result<Exit, SupervisorError> {
    if children.is_empty() {
        return Ok(Exit::AllExited);
    }
    let log_order: Vec<String> = if opts.watch.is_empty() {
        children
            .iter()
            .take(5)
            .map(|(p, _)| p.name.clone())
            .collect()
    } else {
        opts.watch.to_vec()
    };
    let mut tails: Vec<(String, LogTail)> = log_order
        .into_iter()
        .map(|name| (name, LogTail::new(opts.log_lines.max(1))))
        .collect();

    let mut terminal = if io::stdout().is_terminal() {
        match init_terminal() {
            Ok(term) => Some(term),
            Err(source) => {
                eprintln!(
                    "devbox: failed to start the live dashboard; falling back to plain output: {source}"
                );
                None
            }
        }
    } else {
        None
    };
    let interactive = terminal.is_some();

    let mut prev_samples: HashMap<u32, (ProcessSample, Instant)> = HashMap::new();
    let mut exited: HashMap<String, i32> = HashMap::new();
    let mut interrupted = false;
    let mut wait_err: Option<SupervisorError> = None;
    let started = Instant::now();

    let mut listening_cache: HashMap<u32, Vec<String>> = HashMap::new();
    let mut last_listen_scan = Instant::now() - LISTEN_SCAN_INTERVAL;

    'tick: loop {
        if opts.stop.load(Ordering::SeqCst) || (interactive && ctrl_c_pressed()) {
            interrupted = true;
            break;
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
                    wait_err = Some(SupervisorError::Wait {
                        name: process.name.clone(),
                        source,
                    });
                    break 'tick;
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
        let listening = if last_listen_scan.elapsed() >= LISTEN_SCAN_INTERVAL {
            last_listen_scan = Instant::now();
            listening_cache = listening_endpoints(&running_pids);
            &listening_cache
        } else {
            listening_cache.retain(|pid, _| running_pids.contains(pid));
            &listening_cache
        };

        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as f64;
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

        match &mut terminal {
            Some(term) => render_tui(
                term, children, &exited, &metrics, listening, &tails, started,
            ),
            None => render_plain(children, &exited, &metrics, listening, &tails, started),
        }

        let all_done = children.iter().all(|(p, _)| exited.contains_key(&p.name));
        if all_done {
            break;
        }
        std::thread::sleep(opts.refresh);
    }

    restore(terminal.as_ref());

    if let Some(err) = wait_err {
        return Err(err);
    }
    if interrupted {
        println!("\rdevbox: received interrupt, stopping services\n");
        return Ok(Exit::Interrupted);
    }
    Ok(Exit::AllExited)
}

type UiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

fn init_terminal() -> io::Result<UiTerminal> {
    ratatui::crossterm::terminal::enable_raw_mode()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore(terminal: Option<&UiTerminal>) {
    if terminal.is_some() {
        let _ = ratatui::crossterm::terminal::disable_raw_mode();
        let _ = ratatui::crossterm::execute!(
            io::stdout(),
            ratatui::crossterm::cursor::Show,
            ratatui::crossterm::terminal::Clear(ratatui::crossterm::terminal::ClearType::All)
        );
    }
}

/// Returns true once a Ctrl+C key press is seen in the input queue. In raw
/// mode the terminal no longer turns Ctrl+C into a signal, so it must be
/// consumed as an ordinary key event.
fn ctrl_c_pressed() -> bool {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    loop {
        match event::poll(Duration::ZERO) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) => {
                    if key.kind == KeyEventKind::Press
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
                    {
                        return true;
                    }
                }
                Ok(_) => {}
                Err(_) => return false,
            },
            _ => return false,
        }
    }
}

/// One row of the services table, shared by the TUI and plain renderers.
struct ServiceRow {
    name: String,
    status: String,
    running: bool,
    exit_code: Option<i32>,
    pid: u32,
    parent_pid: u32,
    cpu: String,
    memory: String,
    listening: String,
}

fn service_rows(
    children: &[(Process, Child)],
    exited: &HashMap<String, i32>,
    metrics: &HashMap<u32, Metrics>,
    listening: &HashMap<u32, Vec<String>>,
) -> Vec<ServiceRow> {
    children
        .iter()
        .map(|(process, _)| {
            let running = !exited.contains_key(&process.name);
            let exit_code = if running {
                None
            } else {
                exited.get(&process.name).copied()
            };
            let status = match exit_code {
                Some(code) => format!("exited ({code})"),
                None => "running".to_string(),
            };
            let m = metrics.get(&process.pid);
            let cpu = m
                .and_then(|m| m.cpu_percent)
                .map(|v| format!("{v:.0}%"))
                .unwrap_or_default();
            let memory = m
                .and_then(|m| m.memory_bytes)
                .map(format_bytes)
                .unwrap_or_default();
            let listening = listening
                .get(&process.pid)
                .map(|e| e.join(" "))
                .unwrap_or_default();
            ServiceRow {
                name: process.name.clone(),
                status,
                running,
                exit_code,
                pid: process.pid,
                parent_pid: process.parent_pid,
                cpu,
                memory,
                listening,
            }
        })
        .collect()
}

fn table_headers() -> Vec<String> {
    [
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
    .collect()
}

fn table_widths(headers: &[String], rows: &[ServiceRow]) -> Vec<Constraint> {
    let mut w: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for r in rows {
        if !w.is_empty() {
            w[0] = w[0].max(r.name.len());
        }
        if w.len() > 1 {
            w[1] = w[1].max(r.status.len());
        }
        if w.len() > 2 {
            w[2] = w[2].max(r.pid.to_string().len());
        }
        if w.len() > 3 {
            w[3] = w[3].max(r.parent_pid.to_string().len());
        }
        if w.len() > 4 {
            w[4] = w[4].max(r.cpu.len());
        }
        if w.len() > 5 {
            w[5] = w[5].max(r.memory.len());
        }
        if w.len() > 6 {
            w[6] = w[6].max(r.listening.len());
        }
    }
    let mut constraints: Vec<Constraint> = w
        .iter()
        .map(|&len| Constraint::Length((len.min(32) as u16) + 2))
        .collect();
    if let Some(last) = constraints.last_mut() {
        *last = Constraint::Fill(1);
    }
    constraints
}

fn is_numeric_col(index: usize) -> bool {
    matches!(index, 2..=5)
}

fn align_cell(content: String, right: bool) -> Cell<'static> {
    if right {
        Cell::from(Line::from(content).alignment(Alignment::Right))
    } else {
        Cell::from(content)
    }
}

fn build_title(
    width: u16,
    pid: u32,
    running: usize,
    exited: usize,
    started: Instant,
) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            " devbox up ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("pid {pid}"), Style::default().fg(Color::DarkGray)),
    ];
    let count_text = format!("  {running} running  {exited} exited");
    let count_color = if exited == 0 {
        Color::Green
    } else {
        Color::Yellow
    };
    spans.push(Span::styled(count_text, Style::default().fg(count_color)));
    spans.push(Span::styled(
        format!("  up {}", format_uptime(started.elapsed())),
        Style::default().fg(Color::DarkGray),
    ));

    let hint = "  [Ctrl+C] stop ";
    let used: usize = spans.iter().map(|s| s.content.len()).sum();
    let pad = (width as usize).saturating_sub(used + hint.len());
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    spans.push(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    Line::from(spans)
}

fn render_tui(
    terminal: &mut UiTerminal,
    children: &[(Process, Child)],
    exited: &HashMap<String, i32>,
    metrics: &HashMap<u32, Metrics>,
    listening: &HashMap<u32, Vec<String>>,
    tails: &[(String, LogTail)],
    started: Instant,
) {
    let headers = table_headers();
    let rows = service_rows(children, exited, metrics, listening);
    let width_constraints = table_widths(&headers, &rows);
    let running_count = rows.iter().filter(|r| r.running).count();
    let exited_count = rows.len() - running_count;
    let pid = std::process::id();

    let _ = terminal.draw(|f| {
        let area = f.area();
        let table_height = (rows.len() as u16 + 4)
            .min(40)
            .min(area.height.saturating_sub(2));
        let areas = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(table_height),
            Constraint::Min(1),
        ])
        .split(area);
        let (title_area, table_area, logs_area) = (areas[0], areas[1], areas[2]);

        let title = build_title(area.width, pid, running_count, exited_count, started);
        f.render_widget(Paragraph::new(title), title_area);

        let header_row = Row::new(
            headers
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    align_cell(h.clone(), is_numeric_col(i)).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .bottom_margin(1);

        let body: Vec<Row> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let status_style = if r.running {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else if r.exit_code == Some(0) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Red)
                };
                Row::new(vec![
                    Cell::from(r.name.clone()).style(
                        Style::default()
                            .fg(service_color(i))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Cell::from(r.status.clone()).style(status_style),
                    align_cell(r.pid.to_string(), true).style(Style::default().fg(Color::DarkGray)),
                    align_cell(r.parent_pid.to_string(), true)
                        .style(Style::default().fg(Color::DarkGray)),
                    align_cell(r.cpu.clone(), true).style(Style::default().fg(Color::Magenta)),
                    align_cell(r.memory.clone(), true).style(if r.memory.is_empty() {
                        Style::default()
                    } else {
                        Style::default().fg(Color::Magenta)
                    }),
                    Cell::from(r.listening.clone()).style(Style::default().fg(Color::Blue)),
                ])
            })
            .collect();

        let table = Table::new(body, width_constraints)
            .header(header_row)
            .block(Block::bordered().title(" services "))
            .column_spacing(2);
        f.render_widget(table, table_area);

        if tails.is_empty() || logs_area.height < 2 {
            return;
        }
        let chunks = Layout::vertical(
            tails
                .iter()
                .map(|_| Constraint::Ratio(1, tails.len() as u32))
                .collect::<Vec<_>>(),
        )
        .split(logs_area);
        for ((i, (name, tail)), chunk) in tails.iter().enumerate().zip(chunks.iter()) {
            let color = service_color(i);
            let block = Block::default()
                .border_style(Style::default().fg(color))
                .title(format!(" {name} "));
            if tail.render().next().is_none() {
                f.render_widget(
                    Paragraph::new(
                        Line::from(" waiting for output...")
                            .style(Style::default().fg(Color::DarkGray)),
                    )
                    .block(block),
                    *chunk,
                );
                continue;
            }
            let lines: Vec<Line> = tail.render().map(Line::from).collect();
            let text = Text::from(lines);
            let visible = chunk.height.saturating_sub(2).max(1);
            let content_width = chunk.width.saturating_sub(2).max(1);
            let total_rows: usize = text
                .lines
                .iter()
                .map(|l| wrapped_rows(l, content_width))
                .sum();
            let scroll = total_rows.saturating_sub(visible as usize) as u16;
            f.render_widget(
                Paragraph::new(text)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                *chunk,
            );
        }
    });
}

fn render_plain(
    children: &[(Process, Child)],
    exited: &HashMap<String, i32>,
    metrics: &HashMap<u32, Metrics>,
    listening: &HashMap<u32, Vec<String>>,
    tails: &[(String, LogTail)],
    started: Instant,
) {
    println!();
    println!(
        "--- devbox dashboard (up {}) ---",
        format_uptime(started.elapsed())
    );
    println!("devbox running with pid: {}", std::process::id());
    println!();

    let headers = table_headers();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for service in service_rows(children, exited, metrics, listening) {
        rows.push(vec![
            service.name,
            service.status,
            service.pid.to_string(),
            service.parent_pid.to_string(),
            service.cpu,
            service.memory,
            service.listening,
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
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0}KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
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
    let list = pids
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
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
    let parts: Vec<u64> = s
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<Vec<u64>>>()?;
    match parts.as_slice() {
        [min, sec] => Some(min * 60 + sec),
        [hr, min, sec] => Some(hr * 3600 + min * 60 + sec),
        _ => None,
    }
}

#[cfg(windows)]
fn listening_endpoints(roots: &[u32]) -> HashMap<u32, Vec<String>> {
    let out: HashMap<u32, Vec<String>> = HashMap::new();
    if roots.is_empty() {
        return out;
    }
    let script = r#"
Get-CimInstance Win32_Process | ForEach-Object { "P,$($_.ProcessId),$($_.ParentProcessId)" };
Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | ForEach-Object { "L,$($_.OwningProcess),$($_.LocalAddress):$($_.LocalPort)" }
"#;
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut sockets: HashMap<u32, Vec<String>> = HashMap::new();
    for line in run_powershell(script).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, ',');
        match parts.next() {
            Some("P") => {
                let pid = parts.next().and_then(|s| s.trim().parse::<u32>().ok());
                let ppid = parts.next().and_then(|s| s.trim().parse::<u32>().ok());
                if let (Some(pid), Some(ppid)) = (pid, ppid) {
                    children.entry(ppid).or_default().push(pid);
                }
            }
            Some("L") => {
                let pid = parts.next().and_then(|s| s.trim().parse::<u32>().ok());
                let endpoint = parts.next();
                if let (Some(pid), Some(endpoint)) = (pid, endpoint) {
                    sockets
                        .entry(pid)
                        .or_default()
                        .push(format_endpoint(endpoint.trim()));
                }
            }
            _ => {}
        }
    }
    endpoints_for_descendants(roots, &children, &sockets)
}

#[cfg(unix)]
fn listening_endpoints(roots: &[u32]) -> HashMap<u32, Vec<String>> {
    let out: HashMap<u32, Vec<String>> = HashMap::new();
    if roots.is_empty() {
        return out;
    }
    let Ok(tree) = Command::new("ps").args(["-eo", "pid=,ppid="]).output() else {
        return out;
    };
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for line in String::from_utf8_lossy(&tree.stdout).lines() {
        let mut fields = line.split_whitespace();
        let pid = fields.next().and_then(|s| s.parse::<u32>().ok());
        let ppid = fields.next().and_then(|s| s.parse::<u32>().ok());
        if let (Some(pid), Some(ppid)) = (pid, ppid) {
            children.entry(ppid).or_default().push(pid);
        }
    }
    let Ok(output) = Command::new("ss").args(["-H", "-tlnp"]).output() else {
        return out;
    };
    let mut sockets: HashMap<u32, Vec<String>> = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        let Some(local) = cols.get(3) else {
            continue;
        };
        for pid in parse_ss_pids(line) {
            sockets
                .entry(pid)
                .or_default()
                .push(format_endpoint(local));
        }
    }
    endpoints_for_descendants(roots, &children, &sockets)
}

/// Collects the listening endpoints owned by each root PID or any of its
/// descendants. Services such as `dotnet run` launch the process that actually
/// binds the port as a child, so scanning only the direct PID would miss it.
fn endpoints_for_descendants(
    roots: &[u32],
    children: &HashMap<u32, Vec<u32>>,
    sockets: &HashMap<u32, Vec<String>>,
) -> HashMap<u32, Vec<String>> {
    let mut out: HashMap<u32, Vec<String>> = HashMap::new();
    for &root in roots {
        let mut stack = vec![root];
        let mut seen = HashSet::from([root]);
        let mut endpoints: Vec<String> = Vec::new();
        while let Some(pid) = stack.pop() {
            if let Some(owned) = sockets.get(&pid) {
                endpoints.extend(owned.iter().cloned());
            }
            if let Some(kids) = children.get(&pid) {
                for &kid in kids {
                    if seen.insert(kid) {
                        stack.push(kid);
                    }
                }
            }
        }
        endpoints.sort_unstable();
        endpoints.dedup();
        if !endpoints.is_empty() {
            out.insert(root, endpoints);
        }
    }
    out
}

/// Extracts every `pid=` value from an `ss -tlnp` `users:` payload, which can
/// list multiple owning processes per socket.
#[cfg(unix)]
fn parse_ss_pids(line: &str) -> Vec<u32> {
    line.split("pid=")
        .skip(1)
        .filter_map(|rest| {
            let digits: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            digits.parse::<u32>().ok()
        })
        .collect()
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
        if len <= self.pos {
            return;
        }
        if file.seek(SeekFrom::Start(self.pos)).is_err() {
            return;
        }
        let to_read = len.saturating_sub(self.pos).min(MAX_LOG_BYTES) as usize;
        let mut buf = vec![0u8; to_read];
        let Ok(bytes_read) = file.read(&mut buf) else {
            return;
        };
        buf.truncate(bytes_read);
        self.pos += bytes_read as u64;
        for line in String::from_utf8_lossy(&buf).split('\n') {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            let mut line = line.to_string();
            if line.chars().count() > MAX_LINE_CHARS {
                line = line.chars().take(MAX_LINE_CHARS).collect();
                line.push('\u{2026}');
            }
            self.lines.push_back(line);
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
                env_file: None,
                environment: BTreeMap::new(),
                enabled: true,
            }
        } else {
            Service {
                command: "sh".into(),
                args: vec!["-c".into(), "echo hello".into()],
                cwd: None,
                env_file: None,
                environment: BTreeMap::new(),
                enabled: true,
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
                env_file: None,
                environment: BTreeMap::new(),
                enabled: true,
            }
        } else {
            Service {
                command: "sleep".into(),
                args: vec!["60".into()],
                cwd: None,
                env_file: None,
                environment: BTreeMap::new(),
                enabled: true,
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
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(2048), "2KB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0MB");
        assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0GB");
    }

    #[test]
    fn format_endpoint_maps_loopback() {
        assert_eq!(format_endpoint("127.0.0.1:2009"), "localhost:2009");
        assert_eq!(format_endpoint("0.0.0.0:80"), "*:80");
        assert_eq!(format_endpoint("10.0.0.5:4041"), "10.0.0.5:4041");
    }

    #[test]
    fn wrapped_rows_counts_word_wrap_height() {
        let line = Line::from("one two three four five six seven eight nine ten");
        assert_eq!(wrapped_rows(&line, 10), 6);
        assert_eq!(wrapped_rows(&line, 98), 1);

        let long = Line::from("x".repeat(199));
        assert_eq!(wrapped_rows(&long, 10), 20);

        let mut panel = String::from("a".repeat(4096));
        panel.push('\u{2026}');
        let big = Line::from(panel);
        let rows = wrapped_rows(&big, 98);
        assert!((41..=43).contains(&rows), "long line wraps to about 42 rows, got {rows}");
    }

    #[test]
    fn endpoints_include_descendant_ports() {
        let children = HashMap::from([
            (100u32, vec![200u32]),
            (200u32, vec![300u32]),
            (400u32, vec![]),
        ]);
        let sockets = HashMap::from([
            (200u32, vec!["localhost:5000".to_string()]),
            (300u32, vec!["*:5001".to_string()]),
            (500u32, vec!["localhost:9999".to_string()]),
        ]);
        let out = endpoints_for_descendants(&[100, 400], &children, &sockets);

        let root = out.get(&100).expect("root has endpoints");
        assert_eq!(root, &["*:5001", "localhost:5000"]);
        assert!(!out.contains_key(&400), "leaf with no ports is not reported");
        assert!(!out.contains_key(&500));
    }

    #[cfg(unix)]
    #[test]
    fn parse_ss_pids_extracts_all_owners() {
        let line = "LISTEN 0 4096 127.0.0.1:8080 users:((\"dotnet\",pid=123,fd=5),(\"dotnet\",pid=456,fd=6))";
        assert_eq!(parse_ss_pids(line), vec![123, 456]);
        assert!(parse_ss_pids("LISTEN 0 4096 127.0.0.1:8080").is_empty());
    }
}

