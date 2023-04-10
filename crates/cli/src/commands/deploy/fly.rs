use std::{
    fs::{create_dir_all, File},
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use ansi_term::Color;
use anyhow::{anyhow, Result};
use clap::{Arg, ArgAction, Command};
use heck::ToSnakeCase;

use crate::commands::{
    build::build,
    command::{get, get_required, model_file_arg, CommandDefinition},
};

pub(super) struct FlyCommandDefinition {}

impl CommandDefinition for FlyCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("fly")
            .about("Deploy to Fly.io")
            .arg(model_file_arg())
            .arg(
                Arg::new("app-name")
                    .help("The name of the Fly.io application to deploy to")
                    .short('a')
                    .long("app")
                    .required(true)
                    .num_args(1),
            )
            .arg(
                Arg::new("version")
                    .help("The version of application (Dockerfile will use this as tag)")
                    .short('v')
                    .long("version")
                    .required(false)
                    .default_value("latest")
                    .num_args(1),
            )
            .arg(
                Arg::new("env")
                    .help("Environment variables to pass to the application (e.g. -e KEY=VALUE). May be specified multiple times.")
                    .action(ArgAction::Append) // To allow multiple --env flags ("-e k1=v1 -e k2=v2")
                    .short('e')
                    .long("env")
                    .num_args(1),
            )
            .arg(
                Arg::new("env-file").help("Path to a file containing environment variables to pass to the application")
                    .long("env-file")
                    .required(false)
                    .value_parser(clap::value_parser!(PathBuf))
                    .num_args(1),
            )
            .arg(
                Arg::new("use-fly-db")
                    .help("Use database provided by Fly.io")
                    .required(false)
                    .long("use-fly-db")
                    .num_args(0),
            )
    }

    /// Create a fly.toml file, a Dockerfile, and build the docker image. Then provide instructions
    /// on how to deploy the app to Fly.io.
    ///
    /// To avoid clobbering existing files, this command will create a `fly` directory in the same
    /// directory as the model file, and put the `fly.toml` and `Dockerfile` in there.
    fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let model = get_required(matches, "model")?;
        let app_name = get_required(matches, "app-name")?;
        let version = get_required::<Option<_>>(matches, "version")?;
        let envs = matches.get_many("env").map(|env| env.cloned().collect());
        let env_file = get(matches, "env-file");
        let use_fly_db = matches.get_flag("use-fly-db");

        let image_tag = format!("{}:{}", app_name, version.unwrap_or("latest"));

        build(model, false)?;

        // Canonicalize the model path so that when presented with just "filename.exo", we can still
        // get the directory that it's in.
        let model_path = model.canonicalize()?;
        let model_dir = model_path.parent().unwrap();
        let fly_dir = model_dir.join("fly");

        create_dir_all(&fly_dir)?;

        create_fly_toml(&fly_dir, app_name, &image_tag, &env_file, &envs)?;

        create_dockerfile(&fly_dir, &model_path, app_name, use_fly_db)?;

        let docker_build_output = std::process::Command::new("docker")
            .args(["build", "-t", &image_tag, "-f", "fly/Dockerfile", "."])
            .current_dir(model_dir)
            .output()
            .map_err(|err| {
                anyhow!("While trying to invoke `docker` in order to build the docker image: {err}")
            })?;

        if !docker_build_output.status.success() {
            return Err(anyhow!(
                "Docker build failed. Output: {}",
                String::from_utf8_lossy(&docker_build_output.stderr)
            ));
        }

        println!(
            "{}",
            Color::Purple.paint("If not already done so, run `fly auth login` to login.")
        );

        println!(
            "{}",
            Color::Blue
                .italic()
                .paint("\nTo deploy the app for the first time, run:")
        );
        println!(
            "{}",
            Color::Blue.paint(format!("\tcd {}", fly_dir.display()))
        );
        println!(
            "{}",
            Color::Blue.paint(format!("\tfly apps create {}", app_name))
        );
        println!(
            "{}{}",
            Color::Blue.paint(format!(
                "\tfly secrets set --app {} EXO_JWT_SECRET=",
                app_name,
            )),
            Color::Yellow.paint("<your-jwt-secret>")
        );
        if use_fly_db {
            println!(
                "{}",
                Color::Blue.paint(format!("\tfly postgres create --name {}-db", app_name))
            );
            println!(
                "{}",
                Color::Blue.paint(format!(
                    "\tfly postgres attach --app {} {}-db",
                    app_name, app_name
                ))
            );
            println!(
                "\tIn a separate terminal: {}",
                Color::Blue.paint(format!("fly proxy 54321:5432 -a {}-db", app_name))
            );
            let db_name = &app_name.to_snake_case(); // this is how fly.io names the db
            println!(
                "{}{}{}",
                Color::Blue.paint(format!(
                    "\texo schema create ../{} | psql postgres://{db_name}:",
                    model.to_str().unwrap()
                )),
                Color::Blue.paint(format!("@localhost:54321/{db_name}")),
                Color::Yellow.paint("<APP_DATABASE_PASSWORD>"),
            );
        } else {
            println!(
                "{}{}",
                Color::Blue.paint(format!(
                    "\tfly secrets set --app {} EXO_POSTGRES_URL=",
                    app_name
                )),
                Color::Yellow.paint("<your-postgres-url>")
            );
            println!(
                "{}{}",
                Color::Blue.paint(format!(
                    "\texo schema create ../{} | psql ",
                    model.to_str().unwrap()
                )),
                Color::Yellow.paint("<your-postgres-url>")
            );
        }

        println!("{}", Color::Blue.paint("\tfly deploy --local-only"));

        println!(
            "{}",
            Color::Green
                .italic()
                .paint("\nTo deploy a new version of an existing app, run:")
        );
        println!(
            "{}",
            Color::Green.paint(format!("\tcd {}", fly_dir.display()))
        );
        println!("{}", Color::Green.paint("\tfly deploy --local-only"));

        Ok(())
    }
}

static FLY_TOML: &str = include_str!("../templates/fly.toml");
static DOCKERFILE: &str = include_str!("../templates/Dockerfile.fly");

fn create_dockerfile(fly_dir: &Path, model: &Path, app_name: &str, use_fly_db: bool) -> Result<()> {
    let dockerfile_content = DOCKERFILE.replace(
        "<<<MODEL_FILE_NAME>>>",
        model
            .with_extension("")
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
    );
    let dockerfile_content = dockerfile_content.replace("<<<APP_NAME>>>", app_name);

    let extra_env = if use_fly_db {
        "EXO_POSTGRES_URL=${DATABASE_URL}"
    } else {
        ""
    };
    let dockerfile_content = dockerfile_content.replace("<<<EXTRA_ENV>>>", extra_env);

    let mut dockerfile = File::create(fly_dir.join("Dockerfile"))?;
    dockerfile.write_all(dockerfile_content.as_bytes())?;

    Ok(())
}

/// Create a fly.toml file in the fly directory.
/// Replaces the placeholders in the template with the app name and image tag
/// as well as the environment variables.
fn create_fly_toml(
    fly_dir: &Path,
    app_name: &str,
    image_tag: &str,
    env_file: &Option<PathBuf>,
    envs: &Option<Vec<&str>>,
) -> Result<()> {
    let fly_toml_content = FLY_TOML.replace("<<<APP_NAME>>>", app_name);
    let fly_toml_content = fly_toml_content.replace("<<<IMAGE_NAME>>>", image_tag);

    let mut accumulated_env = String::new();

    // First process the env file, if any (so that explicit --env overrides the env file values)
    if let Some(env_file) = &env_file {
        let env_file = File::open(env_file).map_err(|e| {
            anyhow!(
                "Failed to open env file '{}': {}",
                env_file.to_str().unwrap(),
                e
            )
        })?;
        let reader = std::io::BufReader::new(env_file);
        for line in reader.lines() {
            let line = line?;
            accumulate_env(&mut accumulated_env, &line)?;
        }
    }

    for env in envs.iter().flatten() {
        accumulate_env(&mut accumulated_env, env)?;
    }

    let mut fly_toml_file = File::create(fly_dir.join("fly.toml"))?;
    let fly_toml_content = fly_toml_content.replace("<<<ENV_VARS>>>", &accumulated_env);
    fly_toml_file.write_all(fly_toml_content.as_bytes())?;

    Ok(())
}

fn accumulate_env(envs: &mut String, env: &str) -> Result<()> {
    if env.starts_with('#') {
        return Ok(());
    }
    let parts: Vec<_> = env.split('=').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "Invalid env specified. Must be in the form of KEY=VALUE"
        ));
    }
    let key = parts[0].to_string();
    let value = parts[1].to_string();
    envs.push_str(&format!("{}=\"{}\"\n", key, value));

    Ok(())
}
