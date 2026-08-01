use clap::{Parser, Subcommand};
use std::process::ExitCode;

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
    let runtime = runtime::Runtime::new();
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

fn init() -> ExitCode {
    let cwd = std::env::current_dir().expect("determine current directory");
    match workspace::Workspace::init(&cwd) {
        Ok(ws) => {
            println!("Initialized devbox workspace at {}", ws.root().display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("devbox: {err}");
            ExitCode::FAILURE
        }
    }
}
