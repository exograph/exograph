/// Tracing configuration setup.
///
/// Calling the `init` function will initialise a global tracing subscriber based
/// on the values of the `CLAY_TELEMETRY` and `RUST_LOG` environment variables.
/// Possible `CLAY_TELEMETRY` values are `bunyan` and `jaeger`. If the env variable
/// isn't set, no subscriber will be created.
///
/// These are currently only suitable for local debugging but any implementation
/// of Rust's tracing subscriber can be used with the tracing spans built into
/// the system.
///
/// The Bunyan logger prints to stdout and can be piped to the Rust bunyan
/// command line tool (`cargo install bunyan`).
///
/// To use Jaeger, a local server can be started using docker:
///
/// ```shell
/// $ docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 jaegertracing/all-in-one:latest
/// ```
/// Events and spans will be filtered according to the setting of `RUST_LOG`.
/// See the documentation for `EnvFilter` for more information.
use std::{env, io::stdout, process::exit};
use tracing::{log::LevelFilter, subscriber::set_global_default, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter, Registry};

pub fn init() {
    let name = "Claytip";

    if let Ok(variable) = env::var("CLAY_TELEMETRY") {
        match variable.as_str() {
            "bunyan" => init_subscriber(create_bunyan_subscriber(name, stdout)),
            "jaeger" => init_subscriber(create_opentelemetry_jaeger_subscriber(name)),
            _ => {
                eprintln!("Unknown value for CLAY_TELEMETRY: '{variable}'");
                exit(1);
            }
        }
    } else {
        // log to console
        let mut builder = pretty_env_logger::formatted_builder();
        let mut builder = builder
            .filter_level(LevelFilter::Warn)
            // produces a number of traces that are not too relevant to claytip
            // suppress INFO traces
            .filter_module("tracing_actix_web", LevelFilter::Warn)
            .filter_module("actix_server::worker", LevelFilter::Warn);

        if let Ok(rust_log) = std::env::var("CLAY_CONSOLE_LOG") {
            builder = builder.parse_filters(&rust_log);
        }

        builder.init();
    }
}

fn create_bunyan_subscriber<S>(name: &str, make_writer: S) -> impl Subscriber + Send + Sync
where
    S: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let formatting_layer = BunyanFormattingLayer::new(name.to_string(), make_writer);
    Registry::default()
        .with(env_filter())
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

fn create_opentelemetry_jaeger_subscriber(name: &str) -> impl Subscriber + Send + Sync {
    // Install a new OpenTelemetry trace pipeline
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name(name)
        .install_simple()
        .expect("Failed to install jaeger pipeline");

    // Create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Use the tracing subscriber `Registry`, or any other subscriber
    // that impls `LookupSpan`
    Registry::default().with(env_filter()).with(telemetry)
}

// Create a filter for spans and events based on the setting of `RUST_LOG`
// or default to `info`.
fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    // Converts log calls to tracing events
    tracing_log::LogTracer::init().expect("init logger failed");
    set_global_default(subscriber).expect("Failed to set global subscriber");
}
