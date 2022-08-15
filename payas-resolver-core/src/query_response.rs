use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub body: QueryResponseBody,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum QueryResponseBody {
    Json(JsonValue),
    Raw(Option<String>),
}

impl QueryResponseBody {
    pub fn to_json(&self) -> Result<JsonValue, serde_json::Error> {
        match &self {
            QueryResponseBody::Json(val) => Ok(val.clone()),
            QueryResponseBody::Raw(raw) => {
                if let Some(raw) = raw {
                    serde_json::from_str(raw)
                } else {
                    Ok(JsonValue::Null)
                }
            }
        }
    }
}
