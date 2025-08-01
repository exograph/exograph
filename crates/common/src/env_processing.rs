use std::{path::PathBuf, sync::Arc};

use colored::Colorize;
use exo_env::{CompositeEnvironment, DotEnvironment, Environment, SystemEnvironment};

/// Describe the env processing for a command.
///
/// - `Process(Some(env))`: Process env files and use the provided value to pick the env file to use.
/// - `Process(None)`: Process env-agnostic files (.env and .env.local)
/// - `DoNotProcess`: Do not process env files at all (for example, with the update/new/build commands)
pub enum EnvProcessing {
    Process(Option<String>),
    DoNotProcess,
}

impl EnvProcessing {
    pub fn load_env(&self) -> impl Environment + Send + Sync + 'static {
        // Files in order of precedence
        let mut env_files: Vec<PathBuf> = vec![];

        let mut push_env_file = |file: &str| {
            let file_path = PathBuf::from(file);
            if file_path.exists() {
                println!("Loading env file: {}", file.blue());
                env_files.push(file_path);
            }
        };

        if let EnvProcessing::Process(exo_env) = self {
            if let Some(exo_env) = &exo_env {
                push_env_file(&format!(".env.{}.local", exo_env));
            }

            push_env_file(".env.local");

            if let Some(exo_env) = &exo_env {
                push_env_file(&format!(".env.{}", exo_env));
            }

            push_env_file(".env");
        }

        let mut envs: Vec<Arc<dyn Environment>> = vec![Arc::new(SystemEnvironment)];

        for env_file in env_files.iter() {
            envs.push(Arc::new(DotEnvironment::new(env_file)));
        }

        CompositeEnvironment::new(envs)
    }
}
