use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Debug, Deserialize)]
pub struct OperationsPayload {
    #[serde(rename = "operationName")]
    pub operation_name: Option<String>,
    pub query: String,
    pub variables: Option<Map<String, Value>>,
}
