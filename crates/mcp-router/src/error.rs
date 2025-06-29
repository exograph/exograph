#[derive(thiserror::Error, Debug)]
pub enum McpRouterError {
    #[error("Invalid protocol version: {0}")]
    InvalidProtocolVersion(String),
}
