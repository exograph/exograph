use std::{
    fs::{create_dir_all, File},
    io::{BufRead, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use ansi_term::Color;
use anyhow::{anyhow, Result};
use heck::ToSnakeCase;

use crate::commands::{build::build, command::Command};

static FLY_TOML: &str = include_str!("../templates/fly.toml");
static DOCKERFILE: &str = include_str!("../templates/Dockerfile.fly");

/// Deploy the app to Fly.io
pub struct DeployFlyCommand {
    pub model: PathBuf,
    pub app_name: String,
    pub version: String,
    pub envs: Option<Vec<String>>,
    pub env_file: Option<PathBuf>,
    pub use_fly_db: bool,
}

impl DeployFlyCommand {
    fn image_tag(&self) -> String {
        format!("{}:{}", self.app_name, self.version)
    }
}

impl Command for DeployFlyCommand {
    /// Create a fly.toml file, a Dockerfile, and build the docker image. Then provide instructions
    /// on how to deploy the app to Fly.io.
    ///
    /// To avoid clobbering existing files, this command will create a `fly` directory in the same
    /// directory as the model file, and put the `fly.toml` and `Dockerfile` in there.
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        build(&self.model, None, false)?;

        // Canonicalize the model path so that when presented with just "filename.exo", we can still
        // get the directory that it's in.
        let model_path = self.model.canonicalize()?;
        let model_dir = model_path.parent().unwrap();
        let fly_dir = model_dir.join("fly");

        create_dir_all(&fly_dir)?;

        create_fly_toml(&fly_dir, self)?;

        create_dockerfile(&fly_dir, self)?;

        let docker_build_output = std::process::Command::new("docker")
            .args([
                "build",
                "-t",
                &self.image_tag(),
                "-f",
                "fly/Dockerfile",
                ".",
            ])
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
            Color::Blue.paint(format!("\tfly apps create {}", self.app_name))
        );
        println!(
            "{}{}",
            Color::Blue.paint(format!(
                "\tfly secrets set --app {} EXO_JWT_SECRET=",
                self.app_name,
            )),
            Color::Yellow.paint("<your-jwt-secret>")
        );
        if self.use_fly_db {
            println!(
                "{}",
                Color::Blue.paint(format!("\tfly postgres create --name {}-db", self.app_name))
            );
            println!(
                "{}",
                Color::Blue.paint(format!(
                    "\tfly postgres attach --app {} {}-db",
                    self.app_name, self.app_name
                ))
            );
            println!(
                "\tIn a separate terminal: {}",
                Color::Blue.paint(format!("fly proxy 54321:5432 -a {}-db", self.app_name))
            );
            let db_name = &self.app_name.to_snake_case();
            println!(
                "{}{}{}",
                Color::Blue.paint(format!(
                    "\texo schema create ../{} | psql postgres://{db_name}:",
                    self.model.to_str().unwrap()
                )),
                Color::Blue.paint(format!("@localhost:54321/{db_name}")),
                Color::Yellow.paint("<APP_DATABASE_PASSWORD>"),
            );
        } else {
            println!(
                "{}{}",
                Color::Blue.paint(format!(
                    "\tfly secrets set --app {} EXO_POSTGRES_URL=",
                    self.app_name
                )),
                Color::Yellow.paint("<your-postgres-url>")
            );
            println!(
                "{}{}",
                Color::Blue.paint(format!(
                    "\texo schema create ../{} | psql ",
                    self.model.to_str().unwrap()
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

fn create_dockerfile(fly_dir: &Path, command: &DeployFlyCommand) -> Result<()> {
    let dockerfile_content = DOCKERFILE.replace(
        "<<<MODEL_FILE_NAME>>>",
        command
            .model
            .with_extension("")
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
    );
    let dockerfile_content = dockerfile_content.replace("<<<APP_NAME>>>", &command.app_name);

    let extra_env = if command.use_fly_db {
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
fn create_fly_toml(fly_dir: &Path, command: &DeployFlyCommand) -> Result<()> {
    let fly_toml_content = FLY_TOML.replace("<<<APP_NAME>>>", &command.app_name);
    let fly_toml_content = fly_toml_content.replace("<<<IMAGE_NAME>>>", &command.image_tag());

    let mut accumulated_env = String::new();

    // First process the env file, if any (so that explicit --env overrides the env file values)
    if let Some(env_file) = &command.env_file {
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

    for env in command.envs.iter().flatten() {
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
