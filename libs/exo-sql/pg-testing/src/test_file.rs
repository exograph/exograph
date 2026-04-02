use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct SqlTestFile {
    pub(crate) query: QuerySection,
    pub(crate) expect: ExpectSection,
}

#[derive(Debug, Deserialize)]
pub(crate) struct QuerySection {
    pub(crate) statement: String,
    #[serde(default)]
    pub(crate) params: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExpectSection {
    #[serde(default)]
    pub(crate) unordered_paths: Vec<String>,
    pub(crate) result: serde_json::Value,
}
