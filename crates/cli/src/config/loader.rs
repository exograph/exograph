use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::model::Config;

fn load_config_from_file(path: &Path) -> Result<Config> {
    let toml_str = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("Failed to read file '{}': {}", path.display(), e))?;
    let config: Config = toml::from_str(&toml_str)
        .map_err(|e| anyhow!("Failed to parse TOML file '{}': {}", path.display(), e))?;
    Ok(config)
}

pub fn load_config() -> Result<Config> {
    let config_path = Path::new("exo.toml");

    if !config_path.exists() {
        return Ok(Config::default());
    }

    load_config_from_file(config_path)
}

#[cfg(test)]
mod tests {
    use crate::config::model::WatchConfig;

    use super::*;

    #[test]
    fn test_load_config() {
        let config = load_test_config("empty").unwrap();
        assert_eq!(config, Config { watch: None });
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
                watch: Some(WatchConfig {
                    before: Some(vec![
                        "echo 'before1'".to_string(),
                        "echo 'before2'".to_string()
                    ]),
                    after: Some(vec![
                        "echo 'after1'".to_string(),
                        "echo 'after2'".to_string()
                    ])
                })
            }
        );
    }

    #[test]
    fn test_load_watch_after_config() {
        let config = load_test_config("watch-after").unwrap();
        assert_eq!(
            config,
            Config {
                watch: Some(WatchConfig {
                    before: None,
                    after: Some(vec![
                        "echo 'after1'".to_string(),
                        "echo 'after2'".to_string()
                    ])
                })
            }
        );
    }
}
