use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub(crate) fn setup() -> WorkerGuard {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .with_env_var("EXO_LSP_LOG")
        .from_env_lossy();

    let log_dir = std::env::var("EXO_LSP_LOG_DIR")
        .map(|url_string| {
            let url = url::Url::parse(&url_string).unwrap();
            url.to_file_path().unwrap()
        })
        .unwrap_or_else(|_| std::env::current_exe().unwrap().join("logs"));

    eprintln!("log dir: {:?}", log_dir);

    std::fs::create_dir_all(&log_dir).unwrap();

    let file_appender = tracing_appender::rolling::never(log_dir, "lsp.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(filter)
        .with_ansi(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber.");

    guard
}
