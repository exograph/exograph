use anyhow::Result;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

pub(crate) fn setup() -> Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .with_env_var("EXO_LSP_LOG")
        .from_env_lossy();

    let subscriber = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .with_ansi(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}
