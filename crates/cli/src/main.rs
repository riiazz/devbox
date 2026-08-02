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

    /// Open an interactive shell inside the DevBox environment
    Shell,

    /// Manage the tool registry
    Tools(ToolsArgs),
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Exec(args) => exec(&args),
        Commands::Init => init(),
        Commands::Install(args) => install(&args),
        Commands::Shell => shell(),
        Commands::Tools(args) => tools(&args),
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

/// Applies `[environment]` from `devbox.toml` and prepends installed tool
/// directories to `PATH`. Returns `Some(ExitCode)` on failure.
fn prepare_runtime(runtime: &mut runtime::Runtime) -> Option<ExitCode> {
    if let Some(config_path) = find_config() {
        match Config::load(&config_path) {
            Ok(config) => apply_environment(runtime, &config),
            Err(err) => {
                eprintln!("devbox: {err}");
                return Some(ExitCode::FAILURE);
            }
        }
    }

    if let Some(tool_dirs) = installed_tool_dirs() {
        let env = runtime.environment_mut();
        for dir in tool_dirs.iter().rev() {
            env.prepend_path(dir);
        }
    }
    None
}

/// Directories containing installed tool executables, highest priority first.
fn installed_tool_dirs() -> Option<Vec<PathBuf>> {
    let ws = workspace::Workspace::discover().ok()?;
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

fn find_config() -> Option<PathBuf> {
    let root = workspace::Workspace::discover().ok()?.root().to_path_buf();
    let path = root.join(config::FILE_NAME);
    path.is_file().then_some(path)
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
