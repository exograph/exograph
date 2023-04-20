// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! # Tracing/Telemetry configuration setup.
//!
//! The server code is instrumented with Rust's `tracing` frawework.
//!
//! Calling the `init` function will initialize a global tracing subscriber based on the values of
//! the `EXO_LOG` environment variable which follows the same conventions as `RUST_LOG`. This will
//! provide console logging.
//!
//! ## OpenTelemetry
//!
//! The system can also export tracing data to an OpenTelemetry compatible system using
//! [standard environment variables](https://opentelemetry.io/docs/concepts/sdk-configuration/otlp-exporter-configuration/)
//!
//! These include:
//!
//! - `OTEL_SERVICE_NAME` to set the name of your service.
//! - `OTEL_EXPORTER_OTLP_ENDPOINT` to set the endpoint to export trace data to.
//! - `OTEL_EXPORTER_OTLP_PROTOCOL` the OTLP version used. Can be `grpc` (the default) or `http/protobuf`.
//! - `OTEL_EXPORTER_OTLP_HEADERS` allows you to set custom headers such as authentication tokens.
//!
//! At least one `OTEL_` prefixed variable must be set to enable OpenTelemetry.
//!
//! To use Jaeger, a local server can be started using docker:
//!
//! ```shell
//! $ docker run -d --name jaeger -e COLLECTOR_OTLP_ENABLED=true -p 16686:16686 -p 4317:4317 -p 4318:4318 jaegertracing/all-in-one:latest
//! ```
//!
use opentelemetry_otlp::WithExportConfig;
use std::str::FromStr;
use tracing_subscriber::{filter::LevelFilter, prelude::*, EnvFilter};

/// Initialize the tracing subscriber.
///
/// Creates a `tracing_subscriber::fmt` layer by default and adds a `tracing_opentelemetry`
/// layer if OpenTelemetry, exporting traces with `opentelemetry_otlp` if any OpenTelemetry
/// environment variables are set.
pub(super) fn init() {
    let fmt_layer = tracing_subscriber::fmt::layer().compact();
    let telemetry_layer =
        create_otlp_tracer().map(|t| tracing_opentelemetry::layer().with_tracer(t));
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .with_env_var("EXO_LOG")
        .from_env_lossy()
        .add_directive(
            "h2=warn"
                .parse()
                .expect("Hard coded directive shouldn't fail"),
        )
        // By default, actix_server logs every request at the INFO level. This ends up printing
        // a lot of information to the console at the start and end such as:
        // 2023-04-20T16:25:35.691768Z  INFO actix_server::builder: starting 12 workers
        // 2023-04-20T16:25:35.693156Z  INFO actix_server::server: Actix runtime found; starting in Actix runtime
        // ^C2023-04-20T16:25:39.741331Z  INFO actix_server::server: SIGINT received; starting forced shutdown
        // 2023-04-20T16:25:39.741799Z  INFO actix_server::worker: shutting down idle worker
        // 2023-04-20T16:25:39.741800Z  INFO actix_server::worker: shutting down idle worker
        // 2023-04-20T16:25:39.741804Z  INFO actix_server::worker: shutting down idle worker
        // 2023-04-20T16:25:39.741805Z  INFO actix_server::worker: shutting down idle worker
        // So set the default to WARN.
        .add_directive(
            "actix_server=warn"
                .parse()
                .expect("Hard coded directive shouldn't fail"),
        );

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(telemetry_layer)
        .init();
}

fn create_otlp_tracer() -> Option<opentelemetry::sdk::trace::Tracer> {
    if !std::env::vars().any(|(name, _)| name.starts_with("OTEL_")) {
        return None;
    }
    let protocol = std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL").unwrap_or("grpc".to_string());

    let mut tracer = opentelemetry_otlp::new_pipeline().tracing();
    let headers = parse_otlp_headers_from_env();

    match protocol.as_str() {
        "grpc" => {
            let mut exporter = opentelemetry_otlp::new_exporter()
                .tonic()
                .with_metadata(metadata_from_headers(headers))
                .with_env();

            // Check if we need TLS
            if let Ok(endpoint) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
                if endpoint.starts_with("https") {
                    exporter = exporter.with_tls_config(Default::default());
                }
            }
            tracer = tracer.with_exporter(exporter)
        }
        "http/protobuf" => {
            let exporter = opentelemetry_otlp::new_exporter()
                .http()
                .with_headers(headers.into_iter().collect())
                .with_env();
            tracer = tracer.with_exporter(exporter)
        }
        p => panic!("Unsupported protocol {}", p),
    };

    // Use the simple exporter if running the integration tests and using
    // opentelemetry. Otherwise the test server will be killed before the batched
    // spans are exported.
    // Some(tracer.install_simple().unwrap())
    Some(tracer.install_batch(opentelemetry::runtime::Tokio).unwrap())
}

fn metadata_from_headers(headers: Vec<(String, String)>) -> tonic::metadata::MetadataMap {
    use tonic::metadata;

    let mut metadata = metadata::MetadataMap::new();
    headers.into_iter().for_each(|(name, value)| {
        let value = value
            .parse::<metadata::MetadataValue<metadata::Ascii>>()
            .expect("Header value invalid");
        metadata.insert(metadata::MetadataKey::from_str(&name).unwrap(), value);
    });
    metadata
}

fn parse_otlp_headers_from_env() -> Vec<(String, String)> {
    let mut headers = Vec::new();

    if let Ok(hdrs) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") {
        hdrs.split_terminator(',')
            .filter(|h| !h.is_empty())
            .map(|header| {
                header
                    .split_once('=')
                    .expect("Header should contain '=' character")
            })
            .for_each(|(name, value)| headers.push((name.to_owned(), value.to_owned())));
    }
    headers
}
