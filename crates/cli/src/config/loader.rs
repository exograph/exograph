use std::path::Path;

use anyhow::{Result, anyhow};
use core_plugin_shared::profile::SchemaProfiles;
use semver::VersionReq;

use crate::config::model::Config;

use serde::Deserialize;

use super::{
    mcp::SchemaProfilesSer,
    model::{ExographConfig, WatchConfig},
};

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfigSer {
    pub exograph: Option<ExographSer>,
    pub build: Option<WatchCommandSer>,
    pub dev: Option<WatchCommandSer>,
    pub yolo: Option<WatchCommandSer>,
    pub mcp: Option<SchemaProfilesSer>,
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
            mcp: config.mcp.map(SchemaProfiles::try_from).transpose()?,
        })
    }
}

impl TryFrom<ExographSer> for ExographConfig {
    type Error = anyhow::Error;

    fn try_from(config: ExographSer) -> Result<Self, Self::Error> {
        let version_req_str = config.version;

        let version_req = match version_req_str {
            Some(version_req_str) => {
                let version_req = VersionReq::parse(&version_req_str)
                    .map_err(|_| anyhow!("Invalid version: {}", version_req_str))?;

                let mut comparators = version_req.comparators;

                // Match the behavior of npm/yarn/pnpm, where `1.2.3` is the same as `=1.2.3` (while the server crate treats `1.2.3` as `^1.2.3`)
                // See, https://github.com/dtolnay/semver/issues/311 (and if that is fixed, remove this code)

                comparators
                    .iter_mut()
                    .zip(version_req_str.split(','))
                    .for_each(|(comparator, part)| {
                        let part = part.trim();
                        if !part.starts_with('^') && comparator.op == semver::Op::Caret {
                            comparator.op = semver::Op::Exact;
                        }
                    });

                Some(VersionReq { comparators })
            }
            None => None,
        };

        Ok(ExographConfig {
            version: version_req,
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
    use std::collections::HashMap;

    use core_plugin_shared::profile::{InclusionExclusion, OperationSet, SchemaProfile};
    use semver::Version;
    use wildmatch::WildMatch;

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
                mcp: None,
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
                mcp: None,
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
                    version: Some(VersionReq::parse("=0.11.1").unwrap()),
                }),
                build: None,
                dev: None,
                yolo: None,
                mcp: None,
            }
        );
    }

    #[test]
    fn test_load_mcp_config() {
        let config = load_test_config("mcp").unwrap();
        assert_eq!(
            config.mcp,
            Some(SchemaProfiles {
                profiles: HashMap::from([
                    (
                        "membership_management".to_string(),
                        SchemaProfile {
                            queries: OperationSet {
                                models: InclusionExclusion {
                                    include: vec![
                                        WildMatch::new("Membership*"),
                                        WildMatch::new("User"),
                                    ],
                                    exclude: vec![
                                        WildMatch::new("Venue"),
                                        WildMatch::new("Concert"),
                                    ],
                                },
                                operations: InclusionExclusion {
                                    include: vec![
                                        WildMatch::new("memberships"),
                                        WildMatch::new("user"),
                                    ],
                                    exclude: vec![WildMatch::new("*Agg")],
                                },
                            },
                            mutations: OperationSet {
                                models: InclusionExclusion {
                                    include: vec![WildMatch::new("Membership*")],
                                    exclude: vec![
                                        WildMatch::new("Venue"),
                                        WildMatch::new("Concert"),
                                    ],
                                },
                                operations: InclusionExclusion {
                                    include: vec![
                                        WildMatch::new("createMembership"),
                                        WildMatch::new("updateMembership"),
                                    ],
                                    exclude: vec![WildMatch::new("deleteMembership")],
                                },
                            },
                        }
                    ),
                    (
                        "concert_management".to_string(),
                        SchemaProfile {
                            queries: OperationSet {
                                models: InclusionExclusion {
                                    include: vec![
                                        WildMatch::new("Concert"),
                                        WildMatch::new("Venue")
                                    ],
                                    exclude: vec![WildMatch::new("Membership*")],
                                },
                                operations: InclusionExclusion {
                                    include: vec![WildMatch::new("concerts")],
                                    exclude: vec![WildMatch::new("*Agg")],
                                },
                            },
                            mutations: OperationSet::none(),
                        }
                    ),
                ]),
            })
        );
    }

    #[test]
    fn version_req() {
        // req_version -> (matching, not_matching)
        let table = HashMap::from([
            ("0.11.1", (vec!["0.11.1"], vec!["0.11.2"])),
            ("=0.11.1", (vec!["0.11.1"], vec!["0.11.2"])),
            (
                "^0.11.1",
                (vec!["0.11.1", "0.11.2"], vec!["0.11.0", "0.12.1"]),
            ),
            (
                "^1.2.3",
                (vec!["1.2.3", "1.2.4", "1.3.0"], vec!["1.2.2", "2.0.0"]),
            ),
            (
                "~1.2.3",
                (vec!["1.2.3", "1.2.4"], vec!["1.2.2", "1.3.0", "2.0.0"]),
            ),
        ]);

        for (req_version_str, (matching, not_matching)) in table {
            let exograph_config = ExographSer {
                version: Some(req_version_str.to_string()),
            };

            let exograph_config = ExographConfig::try_from(exograph_config).unwrap();

            let req_version = exograph_config.version.unwrap();

            for version in matching {
                assert!(
                    req_version.matches(&Version::parse(version).unwrap()),
                    "Should match version: {} for req_version: {}",
                    version,
                    req_version_str
                );
            }

            for version in not_matching {
                assert!(
                    !req_version.matches(&Version::parse(version).unwrap()),
                    "Should not match version: {} for req_version: {}",
                    version,
                    req_version_str
                );
            }
        }
    }
}
