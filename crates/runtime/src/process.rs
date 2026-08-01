use std::io;
use std::process::{Child, Command, ExitStatus, Stdio};

use crate::environment::Environment;

#[derive(Debug)]
pub struct SpawnError {
    pub program: String,
    pub source: io::Error,
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to spawn `{}`: {}", self.program, self.source)
    }
}

impl std::error::Error for SpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

pub fn command(program: &str, args: &[String], env: &Environment) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    env.apply(&mut cmd);
    cmd
}

pub fn spawn(program: &str, args: &[String], env: &Environment) -> Result<Child, SpawnError> {
    command(program, args, env)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|source| SpawnError {
            program: program.to_string(),
            source,
        })
}

pub fn run(program: &str, args: &[String], env: &Environment) -> Result<ExitStatus, SpawnError> {
    let mut child = spawn(program, args, env)?;
    child.wait().map_err(|source| SpawnError {
        program: program.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;

    #[test]
    fn run_returns_status() {
        let env = Environment::from_current();
        let status = run("cargo", &["--version".into()], &env).expect("spawn cargo");
        assert!(status.success());
    }
}
