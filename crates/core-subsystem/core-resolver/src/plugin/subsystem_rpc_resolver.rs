use thiserror::Error;

use crate::QueryResponse;

use async_trait::async_trait;
use common::context::RequestContext;
use http::StatusCode;
use serde::{Deserialize, Serialize};

pub struct SubsystemRpcResponse {
    pub response: QueryResponse,
    pub status_code: StatusCode,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,

    pub id: JsonRpcId,
    pub method: String,
    #[allow(dead_code)]
    pub params: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum JsonRpcId {
    String(String),
    Number(u64),
}

#[async_trait]
pub trait SubsystemRpcResolver: Sync {
    /// The id of the subsystem (for debugging purposes)
    fn id(&self) -> &'static str;

    async fn resolve<'a>(
        &self,
        request_method: &str,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError>;
}

#[derive(Error, Debug)]
pub enum SubsystemRpcError {
    #[error("Invalid JSON")]
    ParseError,

    #[error("Internal error")]
    InternalError,

    #[error("Invalid parameter {0} for {1}")]
    InvalidParams(String, &'static str), // (field name, container type)

    #[error("Invalid method name: {0}")]
    MethodNotFound(String),

    #[error("Invalid JSON-RPC request")]
    InvalidRequest,

    #[error("Not authorized")]
    Authorization,

    #[error("{0}")]
    UserDisplayError(String), // Error message to be displayed to the user (subsystems should hide internal errors through this)

    #[error("No interceptor found")]
    NoInterceptorFound, // Almost certainly a programming error (we asked a wrong subsystem)
}

impl SubsystemRpcError {
    pub fn user_error_message(&self) -> Option<String> {
        match self {
            SubsystemRpcError::ParseError => Some("Invalid JSON".to_string()),
            SubsystemRpcError::InternalError => Some("Internal error".to_string()),
            SubsystemRpcError::InvalidParams(parameter_name, container_type) => Some(format!(
                "Invalid parameter {parameter_name} for {container_type}"
            )),
            SubsystemRpcError::MethodNotFound(method_name) => {
                Some(format!("Method {method_name} not found"))
            }
            SubsystemRpcError::InvalidRequest => Some("Invalid JSON-RPC request".to_string()),
            SubsystemRpcError::Authorization => Some("Not authorized".to_string()),
            SubsystemRpcError::UserDisplayError(message) => Some(message.to_string()),
            SubsystemRpcError::NoInterceptorFound => None,
        }
    }

    pub fn error_code_string(&self) -> &'static str {
        match self {
            SubsystemRpcError::ParseError => "-32700",
            SubsystemRpcError::InternalError => "-32603",
            SubsystemRpcError::InvalidParams(_, _) => "-32602",
            SubsystemRpcError::MethodNotFound(_) => "-32601",
            SubsystemRpcError::InvalidRequest => "-32600",

            SubsystemRpcError::Authorization => "-32000",
            SubsystemRpcError::UserDisplayError(_) => "-32001",
            SubsystemRpcError::NoInterceptorFound => "--32603", // Same as InternalError
        }
    }
}
