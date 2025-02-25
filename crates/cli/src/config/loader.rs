use std::path::Path;

use anyhow::{anyhow, Result};
use semver::VersionReq;

use crate::config::model::Config;

use serde::Deserialize;

use super::model::WatchConfig;

#[derive(Deserialize, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigSer {
    #[serde(rename = "tool-version")]
    pub tool_version: Option<String>,
    pub watch: Option<WatchConfigSer>,
}

#[derive(Deserialize, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct WatchConfigSer {
    pub build: Option<WatchScriptsSer>,
    pub dev: Option<Vec<String>>,
    pub yolo: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct WatchScriptsSer {
    pub before: Option<Vec<String>>,
    pub after: Option<Vec<String>>,
}

impl TryFrom<ConfigSer> for Config {
    type Error = anyhow::Error;

    fn try_from(config: ConfigSer) -> Result<Self, Self::Error> {
        Ok(Config {
            tool_version: config
                .tool_version
                .map(|v| VersionReq::parse(&v).map_err(|_| anyhow!("Invalid version: {}", v)))
                .transpose()?,
            watch: config
                .watch
                .map(WatchConfig::try_from)
                .transpose()?
                .unwrap_or_default(),
        })
    }
}

impl TryFrom<WatchConfigSer> for WatchConfig {
    type Error = anyhow::Error;

    fn try_from(config: WatchConfigSer) -> Result<Self, Self::Error> {
        Ok(WatchConfig {
            before_build: config
                .build
                .as_ref()
                .and_then(|b| b.before.clone())
                .unwrap_or_default(),
            after_build: config
                .build
                .as_ref()
                .and_then(|b| b.after.clone())
                .unwrap_or_default(),
            dev: config.dev.unwrap_or_default(),
            yolo: config.yolo.unwrap_or_default(),
        })
    }
}

fn load_config_from_file(path: &Path) -> Result<Config> {
    let toml_str = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("Failed to read file '{}': {}", path.display(), e))?;
    let config: ConfigSer = toml::from_str(&toml_str)
        .map_err(|e| anyhow!("Failed to parse TOML file '{}': {}", path.display(), e))?;

    config.try_into()
}

pub fn load_config() -> Result<Config> {
    let config_path = Path::new("exo.toml");

    if !config_path.exists() {
        return Ok(Config::default());
    }

    let config = load_config_from_file(config_path)?;

    config.assert_tool_version()?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use crate::config::model::WatchConfig;

    use super::*;

    #[test]
    fn test_load_config() {
        let config = load_test_config("empty").unwrap();
        assert_eq!(
            config,
            Config {
                tool_version: None,
                watch: WatchConfig::default()
            }
        );
    }

    fn load_test_config(name: &str) -> Result<Config> {
        let test_configs_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config/test-configs");
        let file_path = test_configs_dir.join(format!("{}.toml", name));
        load_config_from_file(&file_path)
    }

    #[test]
    fn test_load_watch_config() {
        let config = load_test_config("watch").unwrap();
        assert_eq!(
            config,
            Config {
                tool_version: None,
                watch: WatchConfig {
                    before_build: vec![
                        "echo 'before build 1'".to_string(),
                        "echo 'before build 2'".to_string()
                    ],
                    after_build: vec![
                        "echo 'after build 1'".to_string(),
                        "echo 'after build 2'".to_string()
                    ],
                    dev: vec!["echo 'dev1'".to_string(), "echo 'dev2'".to_string()],
                    yolo: vec!["echo 'yolo1'".to_string(), "echo 'yolo2'".to_string()],
                }
            }
        );
    }

    #[test]
    fn test_load_config_with_version() {
        let config = load_test_config("version").unwrap();
        assert_eq!(
            config,
            Config {
                tool_version: Some(VersionReq::parse("0.11.1").unwrap()),
                watch: WatchConfig::default()
            }
        );
    }
}
