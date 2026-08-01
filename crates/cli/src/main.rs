use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use config::Config;

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
    }
}

fn exec(args: &ExecArgs) -> ExitCode {
    let mut runtime = runtime::Runtime::new();

    if let Some(config_path) = find_config() {
        match Config::load(&config_path) {
            Ok(config) => apply_environment(&mut runtime, &config),
            Err(err) => {
                eprintln!("devbox: {err}");
                return ExitCode::FAILURE;
            }
        }
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
