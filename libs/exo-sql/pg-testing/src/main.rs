use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(name = "exo-sql-test", about = "Run exo-sql integration tests")]
struct Cli {
    /// Directory containing test fixtures (or a single fixture directory)
    #[arg(default_value = ".")]
    dir: PathBuf,

    /// Glob pattern to filter which tests to run
    pattern: Option<String>,

    /// Database backend to test against
    #[arg(long, default_value = "pg")]
    backend: String,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match exo_sql_pg_testing::test_runner::run(&cli.dir, &cli.pattern, &cli.backend).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
