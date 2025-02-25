use std::path::Path;

use anyhow::{anyhow, Result};
use semver::VersionReq;

use crate::config::model::Config;

use serde::Deserialize;

use super::model::{ExographConfig, WatchConfig};

#[derive(Deserialize, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigSer {
    pub exograph: Option<ExographSer>,
    pub build: Option<WatchCommandSer>,
    pub dev: Option<WatchCommandSer>,
    pub yolo: Option<WatchCommandSer>,
}

#[derive(Deserialize, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ExographSer {
    pub version: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct WatchCommandSer {
    #[serde(rename = "after-model-change")]
    pub after_model_change: Option<Vec<String>>,
}

impl TryFrom<ConfigSer> for Config {
    type Error = anyhow::Error;

    fn try_from(config: ConfigSer) -> Result<Self, Self::Error> {
        Ok(Config {
            exograph: config.exograph.map(ExographConfig::try_from).transpose()?,
            build: config.build.map(WatchConfig::try_from).transpose()?,
            dev: config.dev.map(WatchConfig::try_from).transpose()?,
            yolo: config.yolo.map(WatchConfig::try_from).transpose()?,
        })
    }
}

impl TryFrom<ExographSer> for ExographConfig {
    type Error = anyhow::Error;

    fn try_from(config: ExographSer) -> Result<Self, Self::Error> {
        Ok(ExographConfig {
            version: config
                .version
                .map(|v| VersionReq::parse(&v).map_err(|_| anyhow!("Invalid version: {}", v)))
                .transpose()?,
        })
    }
}

impl TryFrom<WatchCommandSer> for WatchConfig {
    type Error = anyhow::Error;

    fn try_from(config: WatchCommandSer) -> Result<Self, Self::Error> {
        Ok(WatchConfig {
            after_model_change: config.after_model_change.unwrap_or_default(),
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
                exograph: None,
                build: None,
                dev: None,
                yolo: None,
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
                exograph: None,
                build: Some(WatchConfig {
                    after_model_change: vec![
                        "echo 'after build 1'".to_string(),
                        "echo 'after build 2'".to_string()
                    ],
                }),
                dev: Some(WatchConfig {
                    after_model_change: vec!["echo 'dev1'".to_string(), "echo 'dev2'".to_string()],
                }),
                yolo: Some(WatchConfig {
                    after_model_change: vec![
                        "echo 'yolo1'".to_string(),
                        "echo 'yolo2'".to_string()
                    ],
                }),
            }
        );
    }

    #[test]
    fn test_load_config_with_version() {
        let config = load_test_config("version").unwrap();
        assert_eq!(
            config,
            Config {
                exograph: Some(ExographConfig {
                    version: Some(VersionReq::parse("0.11.1").unwrap()),
                }),
                build: None,
                dev: None,
                yolo: None,
            }
        );
    }
}
