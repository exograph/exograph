use std::str::FromStr;

use anyhow::Result;

use tokio::io::{stdin, stdout, AsyncWriteExt};
use tokio_util::codec::{FramedRead, LinesCodec};

use tracing_subscriber::EnvFilter;

use futures::StreamExt;

use clap::Parser;

/// A bridge to allow MCP client that support on the stdio protocol to connect to Exograph MCP that supports the new Streamable HTTP protocol
/// (https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2025-03-26/basic/transports.mdx#streamable-http).
///
/// Once most clients support the Streamable HTTP protocol, we can remove this bridge.
///
/// Usage:
///
/// ```bash
/// exo-mcp-bridge --endpoint http://localhost:8080/mcp/stream [--header key=value --header key=value ...] [--cookie key=value --cookie key=value ...]
/// ```
#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    setup_tracing()?;

    let stdin = stdin();
    let mut stdout = stdout();

    let mut reader = FramedRead::new(stdin, LinesCodec::new());

    let endpoint = reqwest::Client::new();

    // Read from stdin one line at a time, and write to stdout
    while let Some(line) = reader.next().await.transpose()? {
        tracing::info!("--> {}", line);

        let mut request = endpoint
            .post(args.endpoint.clone())
            .body(line)
            .header("content-type", "application/json");

        for header in &args.headers {
            request = request.header(header.key.as_str(), header.value.as_str());
        }

        for cookie in &args.cookies {
            request = request.header("cookie", format!("{}={}", cookie.key, cookie.value));
        }

        let response = request.send().await?;

        let status = response.status();

        let response_bytes = response.bytes().await?;

        if !response_bytes.is_empty() {
            let response_text = String::from_utf8_lossy(&response_bytes);

            tracing::info!("<-- {} {}", status, response_text);

            stdout.write_all(&response_bytes).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        } else {
            tracing::info!("<-- {}", status);
        }
    }

    Ok(())
}

#[derive(Parser)]
struct Cli {
    #[clap(short, long)]
    endpoint: String,

    // Headers may be specified multiple times
    #[clap(long = "header")]
    headers: Vec<KeyValue>,

    // Cookies may be specified multiple times
    #[clap(long = "cookie")]
    cookies: Vec<KeyValue>,
}

#[derive(Debug, Clone)]
struct KeyValue {
    key: String,
    value: String,
}

impl FromStr for KeyValue {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (key, value) = s
            .split_once("=")
            .ok_or(anyhow::anyhow!("Invalid key-value pair"))?;

        Ok(Self {
            key: key.to_string(),
            value: value.to_string(),
        })
    }
}

fn setup_tracing() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(false)
        .init();

    tracing::info!("Starting MCP HTTP Bridge");

    Ok(())
}
