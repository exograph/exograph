use anyhow::{anyhow, Result};
use semver::{Version, VersionReq};

#[derive(Debug, PartialEq, Default)]
pub struct Config {
    pub tool_version: Option<VersionReq>,
    pub watch: WatchConfig,
}

#[derive(Debug, PartialEq, Default)]
pub struct WatchConfig {
    pub before: Vec<String>,
    pub after: Vec<String>,
}

impl Config {
    pub fn assert_tool_version(&self) -> Result<()> {
        let current_tool_version = env!("CARGO_PKG_VERSION");
        let current_tool_version = Version::parse(current_tool_version)?;

        if let Some(required_tool_version) = &self.tool_version {
            if !required_tool_version.matches(&current_tool_version) {
                return Err(anyhow!(
                    "Tool version mismatch. Config requires Exograph CLI version {}, but {} is installed.",
                    required_tool_version,
                    current_tool_version
                ));
            }
        }

        Ok(())
    }
}
