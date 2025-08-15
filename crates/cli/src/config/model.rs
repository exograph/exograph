use anyhow::{Result, anyhow};
use core_plugin_shared::profile::SchemaProfiles;
use semver::{Version, VersionReq};

#[derive(Debug, PartialEq, Default)]
pub struct Config {
    pub exograph: Option<ExographConfig>,
    pub build: Option<WatchConfig>,
    pub dev: Option<WatchConfig>,
    pub yolo: Option<WatchConfig>,
    pub mcp: Option<SchemaProfiles>,
}

#[derive(Debug, PartialEq, Default)]
pub struct ExographConfig {
    pub version: Option<VersionReq>,
}

#[derive(Debug, PartialEq, Default)]
pub struct WatchConfig {
    pub after_model_change: Vec<String>,
}

impl Config {
    pub fn assert_tool_version(&self) -> Result<()> {
        if let Some(exograph) = &self.exograph {
            exograph.assert_tool_version()?;
        }

        Ok(())
    }

    pub fn scripts(&self, stage: &WatchStage) -> Vec<String> {
        match stage {
            WatchStage::Build => &self.build,
            WatchStage::Dev => &self.dev,
            WatchStage::Yolo => &self.yolo,
        }
        .as_ref()
        .unwrap_or(&WatchConfig::default())
        .after_model_change
        .clone()
    }
}

#[derive(Debug)]
pub enum WatchStage {
    Build,
    Dev,
    Yolo,
}

impl ExographConfig {
    pub fn assert_tool_version(&self) -> Result<()> {
        let current_tool_version = env!("CARGO_PKG_VERSION");
        let current_tool_version = Version::parse(current_tool_version)?;

        if let Some(required_tool_version) = &self.version
            && !required_tool_version.matches(&current_tool_version)
        {
            return Err(anyhow!(
                "Tool version mismatch. Config requires Exograph CLI version {}, but {} is installed.",
                required_tool_version,
                current_tool_version
            ));
        }

        Ok(())
    }
}
