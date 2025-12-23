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
//! The server code is instrumented with Rust's `tracing` framework.
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
//! EXO_ENABLE_OTEL must be set to `true` to enable OpenTelemetry.
//!
//! To use Jaeger, a local server can be started using docker:
//!
//! ```shell
//! $ docker run -d --name jaeger -e COLLECTOR_OTLP_ENABLED=true -p 16686:16686 -p 4317:4317 -p 4318:4318 jaegertracing/all-in-one:latest
//! ```
//!

use exo_env::Environment;
use http::Uri;
use thiserror::Error;

use opentelemetry_otlp::{ExporterBuildError, SpanExporter, WithTonicConfig};
use opentelemetry_sdk::{
    Resource,
    trace::{SdkTracerProvider, TraceError},
};
use std::str::FromStr;
use tonic::transport::{ClientTlsConfig, Endpoint};
use tracing_subscriber::{EnvFilter, filter::LevelFilter, prelude::*};

use crate::env_const::{EXO_ENABLE_OTEL, EXO_LOG};

/// Initialize the tracing subscriber.
///
/// Creates a `tracing_subscriber::fmt` layer by default and adds a OpenTelemetry layer
/// if any OpenTelemetry environment variables are set, exporting traces with `opentelemetry_otlp`.
pub async fn init(env: &dyn Environment) -> Result<(), OtelError> {
    let telemetry_layer = {
        let oltp_trace_provider = create_oltp_trace_provider(env).await?;

        use opentelemetry::trace::TracerProvider as _;
        let oltp_tracer = oltp_trace_provider.map(|provider| provider.tracer("Exograph"));

        oltp_tracer.map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer))
    };

    let fmt_layer = tracing_subscriber::fmt::layer().compact();
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .with_env_var(EXO_LOG)
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(telemetry_layer)
        .init();

    Ok(())
}

async fn create_oltp_trace_provider(
    env: &dyn Environment,
) -> Result<Option<SdkTracerProvider>, OtelError> {
    if !env.enabled(EXO_ENABLE_OTEL, false)? {
        return Ok(None);
    }
    let protocol = env
        .get("OTEL_EXPORTER_OTLP_PROTOCOL")
        .unwrap_or("grpc".to_string());
    // If a traces-specific endpoint is set, use that instead of the exporter's endpoint
    let endpoint = env
        .get("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|| env.get("OTEL_EXPORTER_OTLP_ENDPOINT"));

    let headers = parse_otlp_headers_from_env(env);
    use opentelemetry_otlp::WithExportConfig;

    let exporter = match protocol.as_str() {
        "grpc" => {
            let mut exporter = SpanExporter::builder()
                .with_tonic()
                .with_metadata(metadata_from_headers(headers));

            if let Some(endpoint) = endpoint {
                // Check if we need TLS
                if endpoint.as_str().starts_with("https://") {
                    exporter =
                        exporter.with_tls_config(ClientTlsConfig::default().with_native_roots());
                } else if endpoint.as_str().starts_with("unix://") {
                    #[cfg(not(unix))]
                    return Err(OtelError::UnsupportedProtocol(
                        "Unix domain sockets are only supported on Linux and MacOS".to_string(),
                    ));
                    #[cfg(unix)]
                    {
                        // In environments such as Fly.io, the endpoint is a unix domain socket during build time.
                        // However, the normal connection logic doesn't work with unix domain sockets (it expects
                        // the "authority" part of the URL to be set, which is not the case here). So we need to
                        // manually connect to the socket.
                        // See: https://github.com/hyperium/tonic/blob/master/examples/src/uds/client.rs

                        let path = endpoint.as_str()["unix://".len()..].to_string(); // skip the unix:// prefix

                        // The url ("any.url") is not used (the connector is used instead)
                        let channel = Endpoint::try_from("any.url")?.connect_with_connector_lazy(
                            tower::service_fn(move |_: Uri| {
                                let path = path.clone();
                                async move {
                                    // Connect to the unix domain socket
                                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(
                                        tokio::net::UnixStream::connect(path).await?,
                                    ))
                                }
                            }),
                        );

                        exporter = exporter.with_channel(channel);
                    }
                } else {
                    exporter = exporter.with_endpoint(endpoint);
                }
            }

            Ok(exporter.build()?)
        }
        "http/protobuf" => {
            use opentelemetry_otlp::WithHttpConfig;
            let mut exporter = SpanExporter::builder()
                .with_http()
                .with_headers(headers.into_iter().collect());

            if let Some(endpoint) = endpoint {
                exporter = exporter.with_endpoint(endpoint);
            }

            Ok(exporter.build()?)
        }
        p => Err(OtelError::UnsupportedProtocol(p.to_string())),
    }?;

    Ok(Some(
        SdkTracerProvider::builder()
            .with_resource(Resource::builder().build())
            .with_batch_exporter(exporter)
            .build(),
    ))
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

fn parse_otlp_headers_from_env(env: &dyn Environment) -> Vec<(String, String)> {
    let mut headers = Vec::new();

    if let Some(hdrs) = env.get("OTEL_EXPORTER_OTLP_HEADERS") {
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

#[derive(Error, Debug)]
pub enum OtelError {
    #[error(transparent)]
    TraceError(#[from] TraceError),

    #[error(transparent)]
    ExporterBuildError(#[from] ExporterBuildError),

    #[error("Unsupported protocol {0}")]
    UnsupportedProtocol(String),

    #[error(transparent)]
    TonicError(#[from] tonic::transport::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    EnvError(#[from] exo_env::EnvError),
}
