use clap::{Parser, Subcommand};
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use config::Config;
use toolchain::ToolRegistry;

#[derive(Debug, Parser)]
#[command(
    name = "devbox",
    version,
    about = "Reproducible development environments"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run a program inside the DevBox environment
    Exec(ExecArgs),

    /// Create the .devbox workspace
    Init,

    /// Download and register a tool
    Install(InstallArgs),

    /// Print service logs
    Logs(LogsArgs),

    /// Open an interactive shell inside the DevBox environment
    Shell,

    /// Show status of the services supervised by `devbox up`
    Status,

    /// Stop services supervised by `devbox up`
    Stop(StopArgs),

    /// Manage the tool registry
    Tools(ToolsArgs),

    /// Start the services defined in [services]
    Up,
}

#[derive(Debug, clap::Args)]
struct InstallArgs {
    /// Tool name
    name: String,

    /// Version (defaults to the tool's default version)
    #[arg(short, long)]
    version: Option<String>,
}

#[derive(Debug, clap::Args)]
struct ToolsArgs {
    #[command(subcommand)]
    command: ToolsCommands,
}

#[derive(Debug, Subcommand)]
enum ToolsCommands {
    /// List registered tools
    List,

    /// Register a tool manually
    Register(RegisterArgs),
}

#[derive(Debug, clap::Args)]
struct RegisterArgs {
    /// Tool name
    name: String,

    /// Tool version
    version: String,

    /// Executable name
    #[arg(long)]
    executable: String,

    /// Installation directory (defaults to `.devbox/tools/`)
    #[arg(long)]
    dir: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
struct ExecArgs {
    /// Program to run
    program: String,

    /// Arguments passed to the program
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    args: Vec<String>,
}

#[derive(Debug, clap::Args)]
struct LogsArgs {
    /// Service name (all services when omitted)
    name: Option<String>,

    /// Number of trailing lines to print
    #[arg(short, long, default_value_t = 100)]
    lines: usize,
}

#[derive(Debug, clap::Args)]
struct StopArgs {
    /// Services to stop (all services when omitted)
    #[arg(trailing_var_arg = true)]
    names: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Exec(args) => exec(&args),
        Commands::Init => init(),
        Commands::Install(args) => install(&args),
        Commands::Logs(args) => logs(&args),
        Commands::Shell => shell(),
        Commands::Status => status(),
        Commands::Stop(args) => stop(&args),
        Commands::Tools(args) => tools(&args),
        Commands::Up => up(),
    }
}

fn install(args: &InstallArgs) -> ExitCode {
    let ws = match workspace::Workspace::discover() {
        Ok(ws) => ws,
        Err(err) => {
            eprintln!("devbox: {err}");
            return ExitCode::FAILURE;
        }
    };
    let registry_path = ws.tools_dir().join(toolchain::REGISTRY_FILE);
    let mut registry = match load_registry(&registry_path) {
        Ok(registry) => registry,
        Err(err) => {
            eprintln!("devbox: {err}");
            return ExitCode::FAILURE;
        }
    };

    let installer = downloader::Installer::new(ws.tools_dir(), ws.cache_dir());
    match installer.install(&mut registry, &args.name, args.version.as_deref()) {
        Ok(tool) => {
            if let Err(err) = registry.save(&registry_path) {
                eprintln!("devbox: {err}");
                return ExitCode::FAILURE;
            }
            println!("Installed {} {}", tool.name, tool.version);
            println!("  executable:  {}", tool.executable);
            println!("  install_dir: {}", tool.install_dir.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}

fn tools(args: &ToolsArgs) -> ExitCode {
    let ws = match workspace::Workspace::discover() {
        Ok(ws) => ws,
        Err(err) => {
            eprintln!("devbox: {err}");
            return ExitCode::FAILURE;
        }
    };
    let path = ws.tools_dir().join(toolchain::REGISTRY_FILE);
    match &args.command {
        ToolsCommands::List => tools_list(&path),
        ToolsCommands::Register(args) => tools_register(&path, args),
    }
}

fn tools_list(path: &std::path::Path) -> ExitCode {
    if !path.is_file() {
        println!("No tools registered.");
        return ExitCode::SUCCESS;
    }
    match toolchain::ToolRegistry::load(path) {
        Ok(registry) => {
            let tools = registry.list();
            if tools.is_empty() {
                println!("No tools registered.");
            } else {
                for tool in tools {
                    println!(
                        "{} {} ({} at {})",
                        tool.name,
                        tool.version,
                        tool.executable,
                        tool.install_dir.display()
                    );
                }
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}

fn load_registry(path: &Path) -> Result<ToolRegistry, toolchain::RegistryError> {
    if path.is_file() {
        ToolRegistry::load(path)
    } else {
        Ok(ToolRegistry::new())
    }
}

fn tools_register(path: &Path, args: &RegisterArgs) -> ExitCode {
    let mut registry = match load_registry(path) {
        Ok(registry) => registry,
        Err(err) => {
            eprintln!("devbox: {err}");
            return ExitCode::FAILURE;
        }
    };

    let install_dir = args
        .dir
        .clone()
        .unwrap_or_else(|| path.parent().expect("registry path has parent").to_path_buf());
    let tool = toolchain::Tool::new(
        &args.name,
        &args.version,
        &args.executable,
        &install_dir,
    );
    registry.register(tool);
    match registry.save(path) {
        Ok(()) => {
            println!(
                "Registered {} {} -> {}",
                args.name,
                args.version,
                install_dir.display()
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}

fn exec(args: &ExecArgs) -> ExitCode {
    let mut runtime = runtime::Runtime::new();
    if let Some(code) = prepare_runtime(&mut runtime) {
        return code;
    }
    match runtime.exec(&args.program, &args.args) {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(code as u8),
            None => ExitCode::FAILURE,
        },
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}

fn shell() -> ExitCode {
    let mut runtime = runtime::Runtime::new();
    if let Some(code) = prepare_runtime(&mut runtime) {
        return code;
    }

    let shell = shell_command();
    if let Ok(ws) = workspace::Workspace::discover() {
        println!("devbox: shell for {} ({} on PATH)", ws.root().display(), shell);
    }
    let code = match runtime.exec(&shell, &[]) {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(code as u8),
            None => ExitCode::FAILURE,
        },
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    };
    println!("devbox: shell exited");
    code
}

/// The interactive shell to spawn: `$SHELL` if set, otherwise PowerShell on
/// Windows and `bash` on Unix.
fn shell_command() -> String {
    if let Some(shell) = std::env::var_os("SHELL")
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
    {
        return shell;
    }
    if cfg!(windows) {
        "powershell.exe".to_string()
    } else {
        "bash".to_string()
    }
}

/// Applies v0.8 environment isolation, then `[environment]` from `devbox.toml`,
/// then prepends installed tool directories to `PATH`. Returns `Some(ExitCode)`
/// on failure.
fn prepare_runtime(runtime: &mut runtime::Runtime) -> Option<ExitCode> {
    let ws = workspace::Workspace::discover().ok();

    if let Some(ws) = &ws {
        apply_isolation(runtime, ws);

        if let Some(config_path) = find_config(ws) {
            match Config::load(&config_path) {
                Ok(config) => apply_environment(runtime, &config),
                Err(err) => {
                    eprintln!("devbox: {err}");
                    return Some(ExitCode::FAILURE);
                }
            }
        }
    }

    if let Some(tool_dirs) = installed_tool_dirs(ws.as_ref()) {
        let env = runtime.environment_mut();
        for dir in tool_dirs.iter().rev() {
            env.prepend_path(dir);
        }
    }
    None
}

/// Points `HOME`, `TMP`, `NUGET_PACKAGES`, and `DOTNET_ROOT` into `.devbox`
/// (v0.8). `DOTNET_ROOT` targets a registered `dotnet` tool when one exists.
fn apply_isolation(runtime: &mut runtime::Runtime, ws: &workspace::Workspace) {
    let mut isolation = runtime::Isolation::from_devbox(&ws.devbox_dir());
    if let Some(tool) = installed_dotnet(ws) {
        isolation.dotnet_root = tool.install_dir.clone();
    }
    for dir in [&isolation.home, &isolation.tmp, &isolation.nuget_packages] {
        std::fs::create_dir_all(dir).ok();
    }
    isolation.apply(runtime.environment_mut());
}

/// The registered `dotnet` tool, if any.
fn installed_dotnet(ws: &workspace::Workspace) -> Option<toolchain::Tool> {
    let path = ws.tools_dir().join(toolchain::REGISTRY_FILE);
    let registry = toolchain::ToolRegistry::load(&path).ok()?;
    registry.get("dotnet").cloned()
}

/// Directories containing installed tool executables, highest priority first.
fn installed_tool_dirs(ws: Option<&workspace::Workspace>) -> Option<Vec<PathBuf>> {
    let ws = ws?;
    let path = ws.tools_dir().join(toolchain::REGISTRY_FILE);
    let registry = toolchain::ToolRegistry::load(&path).ok()?;
    Some(registry.executable_dirs())
}

fn apply_environment(runtime: &mut runtime::Runtime, config: &Config) {
    let env = runtime.environment_mut();
    for (key, value) in &config.environment {
        env.set(key, value);
    }
}

fn find_config(ws: &workspace::Workspace) -> Option<PathBuf> {
    let path = ws.root().join(config::FILE_NAME);
    path.is_file().then_some(path)
}

/// A supervisor rooted in the workspace: state and logs live under
/// `.devbox/workspace/`, relative service working directories resolve against
/// the workspace root.
fn supervisor(ws: &workspace::Workspace) -> supervisor::Supervisor {
    supervisor::Supervisor::new(
        ws.workspace_dir().join(supervisor::STATE_FILE),
        ws.workspace_dir().join("logs"),
        ws.root(),
    )
}

fn require_workspace() -> Result<workspace::Workspace, ExitCode> {
    workspace::Workspace::discover().map_err(|err| {
        eprintln!("devbox: {err}");
        ExitCode::FAILURE
    })
}

fn up() -> ExitCode {
    let ws = match require_workspace() {
        Ok(ws) => ws,
        Err(code) => return code,
    };

    let config_path = ws.root().join(config::FILE_NAME);
    let config = match config::Config::load(&config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("devbox: {err}");
            return ExitCode::FAILURE;
        }
    };
    if config.services.is_empty() {
        eprintln!("devbox: no services defined in the [services] section of devbox.toml");
        return ExitCode::FAILURE;
    }

    let mut runtime = runtime::Runtime::new();
    if let Some(code) = prepare_runtime(&mut runtime) {
        return code;
    }

    let sup = supervisor(&ws);
    sup.stop(None).ok();
    match sup.spawn_all(&config.services, runtime.environment()) {
        Ok(mut children) => {
            println!(
                "devbox: starting {} service(s) for {}",
                children.len(),
                ws.root().display()
            );
            if let Err(err) = sup.monitor(&mut children) {
                eprintln!("devbox: {err}");
                sup.stop(None).ok();
                return ExitCode::FAILURE;
            }
            sup.stop(None).ok();
            println!("devbox: all services stopped");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}

fn status() -> ExitCode {
    let ws = match require_workspace() {
        Ok(ws) => ws,
        Err(code) => return code,
    };
    match supervisor(&ws).status() {
        Ok(list) => {
            if list.is_empty() {
                println!("No services running. Start them with `devbox up`.");
                return ExitCode::SUCCESS;
            }
            println!("{:<12} {:>8}  STATUS", "NAME", "PID");
            for entry in list {
                let status = if entry.running { "running" } else { "stopped" };
                println!("{:<12} {:>8}  {}", entry.name, entry.pid, status);
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}

fn logs(args: &LogsArgs) -> ExitCode {
    let ws = match require_workspace() {
        Ok(ws) => ws,
        Err(code) => return code,
    };
    match supervisor(&ws).log_files(args.name.as_deref()) {
        Ok(files) => {
            if files.is_empty() {
                println!("No logs available.");
                return ExitCode::SUCCESS;
            }
            for (name, path) in &files {
                if files.len() > 1 {
                    println!("=== {name} ({}) ===", path.display());
                }
                match supervisor::tail_file(path, args.lines) {
                    Ok(text) => {
                        print!("{text}");
                        if !text.ends_with('\n') {
                            println!();
                        }
                    }
                    Err(err) => {
                        eprintln!("devbox: failed to read `{}`: {err}", path.display());
                        return ExitCode::FAILURE;
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}

fn stop(args: &StopArgs) -> ExitCode {
    let ws = match require_workspace() {
        Ok(ws) => ws,
        Err(code) => return code,
    };
    let names = if args.names.is_empty() {
        None
    } else {
        Some(args.names.as_slice())
    };
    match supervisor(&ws).stop(names) {
        Ok(killed) => {
            if killed.is_empty() {
                println!("No running services to stop.");
            } else {
                println!("Stopped: {}", killed.join(", "));
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}

fn init() -> ExitCode {
    let cwd = std::env::current_dir().expect("determine current directory");
    match workspace::Workspace::init(&cwd) {
        Ok(ws) => {
            let name = cwd
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "devbox".to_string());
            let config = Config {
                workspace: config::Workspace {
                    name: name.clone(),
                },
                environment: Default::default(),
                services: Default::default(),
            };
            let path = ws.root().join(config::FILE_NAME);
            if let Err(err) = config.save(&path) {
                eprintln!("devbox: {err}");
                return ExitCode::FAILURE;
            }
            println!("Initialized devbox workspace at {}", ws.root().display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}
