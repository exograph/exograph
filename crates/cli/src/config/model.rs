use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq, Default)]
pub struct Config {
    pub watch: Option<WatchConfig>,
}

#[derive(Deserialize, Debug, PartialEq, Default)]
pub struct WatchConfig {
    pub before: Option<Vec<String>>,
    pub after: Option<Vec<String>>,
}
