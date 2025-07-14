use exo_env::{EnvError, Environment};

pub const EXO_INTROSPECTION: &str = "EXO_INTROSPECTION";
pub const EXO_INTROSPECTION_LIVE_UPDATE: &str = "EXO_INTROSPECTION_LIVE_UPDATE";

pub const EXO_CORS_DOMAINS: &str = "EXO_CORS_DOMAINS";

pub const EXO_JWT_SECRET: &str = "EXO_JWT_SECRET";
pub const EXO_OIDC_URL: &str = "EXO_OIDC_URL";
pub const EXO_JWT_SOURCE_HEADER: &str = "EXO_JWT_SOURCE_HEADER";
pub const EXO_JWT_SOURCE_COOKIE: &str = "EXO_JWT_SOURCE_COOKIE";

pub const EXO_POSTGRES_URL: &str = "EXO_POSTGRES_URL";
pub const DATABASE_URL: &str = "DATABASE_URL";
pub const EXO_CONNECTION_POOL_SIZE: &str = "EXO_CONNECTION_POOL_SIZE";
pub const EXO_CHECK_CONNECTION_ON_STARTUP: &str = "EXO_CHECK_CONNECTION_ON_STARTUP";

pub const EXO_SERVER_PORT: &str = "EXO_SERVER_PORT";

pub const _EXO_DEPLOYMENT_MODE: &str = "_EXO_DEPLOYMENT_MODE"; // "yolo", "dev", "playground" or "prod" (default)
pub const _EXO_ENFORCE_TRUSTED_DOCUMENTS: &str = "_EXO_ENFORCE_TRUSTED_DOCUMENTS";

pub const _EXO_UPSTREAM_ENDPOINT_URL: &str = "_EXO_UPSTREAM_ENDPOINT_URL";

pub const EXO_PLAYGROUND_HTTP_PATH: &str = "EXO_PLAYGROUND_HTTP_PATH";
pub const EXO_GRAPHQL_HTTP_PATH: &str = "EXO_GRAPHQL_HTTP_PATH";
pub const EXO_REST_HTTP_PATH: &str = "EXO_REST_HTTP_PATH";
pub const EXO_RPC_HTTP_PATH: &str = "EXO_RPC_HTTP_PATH";
pub const EXO_MCP_HTTP_PATH: &str = "EXO_MCP_HTTP_PATH";

pub const EXO_GRAPHQL_ALLOW_MUTATIONS: &str = "EXO_GRAPHQL_ALLOW_MUTATIONS";

pub const EXO_UNSTABLE_ENABLE_REST_API: &str = "EXO_UNSTABLE_ENABLE_REST_API";
pub const EXO_UNSTABLE_ENABLE_RPC_API: &str = "EXO_UNSTABLE_ENABLE_RPC_API";
pub const EXO_ENABLE_MCP: &str = "EXO_ENABLE_MCP";

pub const EXO_WWW_AUTHENTICATE_HEADER: &str = "EXO_WWW_AUTHENTICATE_HEADER";

#[derive(Debug)]
pub enum DeploymentMode {
    Yolo,
    Dev,
    Playground(String), // URL of the GraphQL endpoint to connect to
    Prod,
}

pub fn get_deployment_mode(env: &dyn Environment) -> Result<DeploymentMode, EnvError> {
    let deployment_mode = env.get(_EXO_DEPLOYMENT_MODE);

    match deployment_mode.as_deref() {
        Some("yolo") => Ok(DeploymentMode::Yolo),
        Some("dev") => Ok(DeploymentMode::Dev),
        Some("playground") => {
            let endpoint_url =
                env.get(_EXO_UPSTREAM_ENDPOINT_URL)
                    .ok_or(EnvError::InvalidEnum {
                        env_key: _EXO_UPSTREAM_ENDPOINT_URL,
                        env_value: "".to_string(),
                        message: "Must be set to a valid URL".to_string(),
                    })?;
            Ok(DeploymentMode::Playground(endpoint_url))
        }
        Some("prod") | None => Ok(DeploymentMode::Prod),
        Some(other) => Err(EnvError::InvalidEnum {
            env_key: _EXO_DEPLOYMENT_MODE,
            env_value: other.to_string(),
            message: "Must be one of 'yolo', 'dev', 'playground', or 'prod'".to_string(),
        }),
    }
}

pub fn is_production(env: &dyn Environment) -> bool {
    matches!(get_deployment_mode(env), Ok(DeploymentMode::Prod) | Err(_))
}

#[cfg(not(target_family = "wasm"))]
pub fn get_enforce_trusted_documents(env: &dyn Environment) -> bool {
    env.get(_EXO_ENFORCE_TRUSTED_DOCUMENTS)
        .map(|value| value != "false")
        .unwrap_or(true)
}

pub fn get_playground_http_path(env: &dyn Environment) -> String {
    env.get(EXO_PLAYGROUND_HTTP_PATH)
        .unwrap_or_else(|| "/playground".to_string())
}

pub fn get_graphql_http_path(env: &dyn Environment) -> String {
    env.get(EXO_GRAPHQL_HTTP_PATH)
        .unwrap_or_else(|| "/graphql".to_string())
}

pub fn get_rest_http_path(env: &dyn Environment) -> String {
    env.get(EXO_REST_HTTP_PATH)
        .unwrap_or_else(|| "/api".to_string())
}

pub fn get_rpc_http_path(env: &dyn Environment) -> String {
    env.get(EXO_RPC_HTTP_PATH)
        .unwrap_or_else(|| "/rpc".to_string())
}

pub fn get_mcp_http_path(env: &dyn Environment) -> String {
    env.get(EXO_MCP_HTTP_PATH)
        .unwrap_or_else(|| "/mcp".to_string())
}
