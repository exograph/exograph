use colored::Colorize;
use futures::FutureExt;
use std::path::PathBuf;

use anyhow::anyhow;
use async_trait::async_trait;
use clap::{Arg, ArgMatches, Command};

use anyhow::Result;
use common::env_const::{
    EXO_CHECK_CONNECTION_ON_STARTUP, EXO_CORS_DOMAINS, EXO_INTROSPECTION, EXO_POSTGRES_URL,
    _EXO_DEPLOYMENT_MODE, _EXO_UPSTREAM_ENDPOINT_URL,
};

use crate::{commands::command::get_required, util::watcher};

use super::command::{ensure_exo_project_dir, get, port_arg, CommandDefinition};

/// Run local exograph server in playground-only mode
///
/// This mode is meant to be useful with production deployments of Exograph (which, by default, does
/// not expose introspection or the playground code). This command takes one required argument,
/// `endpoint`, which is the URL of the GraphQL endpoint to connect to.
///
/// In this mode, Exograph fetches the schema from the server started from this command (which has
/// all resolvers, except the schema resolver, disabled) and uses that to run the playground. All
/// GraphQL requests execute against the endpoint specified by the `endpoint` argument.
pub struct PlaygroundCommandDefinition {}

#[async_trait]
impl CommandDefinition for PlaygroundCommandDefinition {
    fn command(&self) -> Command {
        Command::new("playground")
            .about("Run Exograph in playground-only mode")
            .arg(port_arg())
            .arg(
                Arg::new("endpoint")
                    .help("Endpoint URL to connect to (typically http://<remote-url>/graphql)")
                    .long("endpoint")
                    .required(true),
            )
    }

    async fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let port: Option<u32> = get(matches, "port");
        let endpoint_url: String = get_required(matches, "endpoint")?;

        ensure_exo_project_dir(&PathBuf::from("."))?;

        std::env::set_var(EXO_INTROSPECTION, "only");
        // We don't need a database connection in playground mode, but the Postgres resolver
        // currently requires a valid URL to be set (when we fix
        // https://github.com/exograph/exograph/issues/532), we won't need to instantiate the
        // Postgres resolver at all.
        std::env::set_var(EXO_POSTGRES_URL, "postgres://__placeholder");
        std::env::set_var(EXO_CHECK_CONNECTION_ON_STARTUP, "false");

        std::env::set_var(EXO_CORS_DOMAINS, "*");

        std::env::set_var(_EXO_DEPLOYMENT_MODE, "playground");
        std::env::set_var(_EXO_UPSTREAM_ENDPOINT_URL, &endpoint_url);

        let mut server =
            watcher::build_and_start_server(port, &|| async { Ok(()) }.boxed()).await?;

        if let Some(child) = server.as_mut() {
            println!(
                "{} {}",
                "Starting playground server connected to the endpoint at:"
                    .purple()
                    .bold(),
                endpoint_url.blue().bold()
            );
            child.wait().await?;
            Ok(())
        } else {
            Err(anyhow!("Failed to start server"))
        }
    }
}
