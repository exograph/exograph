use std::{
    collections::HashMap,
    fs::{File, create_dir_all},
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;

use anyhow::Result;
use clap::Command;
use colored::Colorize;
use exo_env::Environment;

use crate::commands::{build::build, command::CommandDefinition};
use crate::config::Config;

use common::download::download_file_if_needed;

use super::util::{app_name_arg, app_name_from_args, write_template_file};

pub(super) struct CfWorkerCommandDefinition {}

static WRANGLER_TOML: &str = include_str!("../templates/cf-worker/wrangler.toml");
static DEV_VARS: &str = include_str!("../templates/cf-worker/.dev.vars");

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[async_trait]
impl CommandDefinition for CfWorkerCommandDefinition {
    fn command(&self) -> Command {
        Command::new("cf-worker")
            .about("Deploy to Cloudflare Workers")
            .arg(app_name_arg())
    }

    async fn execute(
        &self,
        matches: &clap::ArgMatches,
        config: &Config,
        _env: Arc<dyn Environment>,
    ) -> Result<()> {
        let app_name: String = app_name_from_args(matches);

        build(false, config).await?; // Build the exo_ir file

        let current_dir = std::env::current_dir()?;
        let cf_worker_dir = current_dir.join("target/cf-worker");
        create_dir_all(&cf_worker_dir)?;

        extract_distribution(&cf_worker_dir).await?;

        create_wrangler_toml(&app_name)?;
        create_dev_vars()?;

        println!("{}", "To test the worker locally:".green());
        println!(
            "\t{}",
            "- Update .dev.vars with environment variables".purple()
        );
        println!("\t- Run: {}", "npx wrangler dev".blue());
        println!(
            "\t- In another terminal, run the command using the url printed earlier:\n\t\t{}",
            "exo playground --endpoint http://localhost:8787".blue()
        );
        println!("\t{}", "- Try queries in the playground".purple());

        println!();

        println!("{}", "To deploy the worker in the cloud:".green());
        println!(
            "\t{}",
            "- Set the environment variables in the Cloudflare dashboard or using".purple()
        );
        println!("\t\t{}", "npx wrangler secret put EXO_POSTGRES_URL".blue());
        println!("\t\t{}", "npx wrangler secret put <other secrets>".blue());
        println!(
            "\t{}",
            "- If you want to enable the Hyperdrive feature".purple()
        );
        println!(
            "\t\t{}",
            "Update Hyperdrive settings in wrangler.toml".purple()
        );

        println!("\t- Run: {}", "npx wrangler deploy".blue());
        println!(
            "\t- In a separate terminal run:\n\t\t{}",
            format!(
                "exo playground --endpoint {}",
                "<url shown in the deploy command>".yellow()
            )
            .blue()
        );
        println!("\t{}", "- Try queries in the playground".purple());

        Ok(())
    }
}

fn create_wrangler_toml(app_name: &str) -> Result<bool> {
    write_template_file(
        Path::new("wrangler.toml"),
        WRANGLER_TOML,
        Some(HashMap::from([("<<<APP_NAME>>>", app_name)])),
    )
}

fn create_dev_vars() -> Result<bool> {
    write_template_file(Path::new(".dev.vars"), DEV_VARS, None)
}

async fn extract_distribution(cf_worker_dir: &PathBuf) -> Result<()> {
    let download_file_name = "exograph-cf-worker-wasm.zip";
    let download_url = format!(
        "https://github.com/exograph/exograph/releases/download/v{CURRENT_VERSION}/{download_file_name}"
    );

    let distribution_zip_path =
        download_file_if_needed(&download_url, "Exograph Cloudflare Worker Distribution").await?;

    let mut distribution_zip_file = zip::ZipArchive::new(File::open(distribution_zip_path)?)?;
    distribution_zip_file.extract(cf_worker_dir)?;

    Ok(())
}
