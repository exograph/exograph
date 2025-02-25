use anyhow::{anyhow, Result};
use semver::{Version, VersionReq};

#[derive(Debug, PartialEq, Default)]
pub struct Config {
    pub tool_version: Option<VersionReq>,
    pub watch: WatchConfig,
}

#[derive(Debug, PartialEq, Default)]
pub struct WatchConfig {
    pub before_build: Vec<String>,
    pub after_build: Vec<String>,

    pub dev: Vec<String>,
    pub yolo: Vec<String>,
}

impl WatchConfig {
    pub fn scripts(&self, stage: &WatchStage) -> Vec<String> {
        match stage {
            WatchStage::Build(WatchStagePos::Before) => self.before_build.clone(),
            WatchStage::Build(WatchStagePos::After) => self.after_build.clone(),
            WatchStage::Dev => self.dev.clone(),
            WatchStage::Yolo => self.yolo.clone(),
        }
    }
}

#[derive(Debug)]
pub enum WatchStagePos {
    Before,
    After,
}

#[derive(Debug)]
pub enum WatchStage {
    Build(WatchStagePos),
    Dev,
    Yolo,
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
