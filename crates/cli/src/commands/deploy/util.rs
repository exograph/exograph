use std::{collections::HashMap, fs::File, io::Write, path::Path};

use anyhow::Result;

use clap::{Arg, ArgMatches};
use colored::Colorize;

use crate::commands::command::get;

pub(super) fn app_name_from_args(matches: &ArgMatches) -> String {
    get(matches, "app-name").unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    })
}

pub(super) fn app_name_arg() -> Arg {
    Arg::new("app-name")
        .help("The name of the application. Defaults to the current directory name.")
        .short('a')
        .long("app")
        .required(false)
        .num_args(1)
}

pub(super) fn write_template_file(
    file_path: impl AsRef<Path>,
    template: &str,
    substitutions: Option<HashMap<&str, &str>>,
) -> Result<bool> {
    let file_path = file_path.as_ref();
    if file_path.exists() {
        println!(
            "{}",
            format!(
                "File '{}' already exists. To regenerate, remove it. Skipping...",
                file_path.display()
            )
            .purple()
        );
        return Ok(false);
    }

    let mut file = File::create(file_path)?;
    match substitutions {
        Some(substitutions) => {
            let content = substitutions
                .iter()
                .fold(template.to_string(), |acc, (key, value)| {
                    acc.replace(key, value)
                });
            file.write_all(content.as_bytes())?;
        }
        None => {
            file.write_all(template.as_bytes())?;
        }
    };

    Ok(true)
}
