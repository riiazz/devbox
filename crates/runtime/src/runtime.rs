use crate::environment::Environment;
use crate::process;

#[derive(Debug)]
pub struct Runtime {
    environment: Environment,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            environment: Environment::from_current(),
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn environment_mut(&mut self) -> &mut Environment {
        &mut self.environment
    }

    pub fn exec(&self, program: &str, args: &[String]) -> Result<std::process::ExitStatus, ExecError> {
        process::run(program, args, &self.environment).map_err(ExecError)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct ExecError(pub process::SpawnError);

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ExecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
