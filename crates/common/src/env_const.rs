use std::sync::Arc;

use exo_env::{CompositeEnvironment, DotEnvironment, EnvError, Environment, SystemEnvironment};

pub const EXO_INTROSPECTION: &str = "EXO_INTROSPECTION";
pub const EXO_INTROSPECTION_LIVE_UPDATE: &str = "EXO_INTROSPECTION_LIVE_UPDATE";

pub const EXO_CORS_DOMAINS: &str = "EXO_CORS_DOMAINS";

pub const EXO_JWT_SECRET: &str = "EXO_JWT_SECRET";
pub const EXO_OIDC_URL: &str = "EXO_OIDC_URL";
pub const EXO_JWT_SOURCE_HEADER: &str = "EXO_JWT_SOURCE_HEADER";
pub const EXO_JWT_SOURCE_COOKIE: &str = "EXO_JWT_SOURCE_COOKIE";

pub const EXO_POSTGRES_URL: &str = "EXO_POSTGRES_URL";
pub const EXO_POSTGRES_READ_WRITE: &str = "EXO_POSTGRES_READ_WRITE";
pub const DATABASE_URL: &str = "DATABASE_URL";
pub const EXO_CONNECTION_POOL_SIZE: &str = "EXO_CONNECTION_POOL_SIZE";
pub const EXO_CHECK_CONNECTION_ON_STARTUP: &str = "EXO_CHECK_CONNECTION_ON_STARTUP";

pub const EXO_SERVER_PORT: &str = "EXO_SERVER_PORT";

pub const EXO_ENV: &str = "EXO_ENV"; // "yolo", "dev", "playground" or "prod" 
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
    Yolo,               // Corresponds to "exo yolo"
    Dev,                // Corresponds to "exo dev"
    Test,               // Corresponds to "exo test"
    Playground(String), // URL of the GraphQL endpoint to connect to (corresponds to "exo playground")
    Production,         // Corresponds to "EXO_ENV=production"
}

impl DeploymentMode {
    pub fn env_key(&self) -> &str {
        match self {
            DeploymentMode::Yolo => "yolo",
            DeploymentMode::Dev => "dev",
            DeploymentMode::Test => "test",
            DeploymentMode::Playground(_) => "playground",
            DeploymentMode::Production => "production",
        }
    }
}

pub fn get_deployment_mode(env: &dyn Environment) -> Result<Option<DeploymentMode>, EnvError> {
    let deployment_mode = env.get(EXO_ENV);

    deployment_mode
        .as_deref()
        .map(|mode| match mode {
            "yolo" => Ok(DeploymentMode::Yolo),
            "dev" => Ok(DeploymentMode::Dev),
            "test" => Ok(DeploymentMode::Test),
            "playground" => {
                let endpoint_url = env.get(_EXO_UPSTREAM_ENDPOINT_URL).ok_or_else(|| {
                    let actual_value = env
                        .get(_EXO_UPSTREAM_ENDPOINT_URL)
                        .unwrap_or_else(|| "<unset>".to_string());
                    EnvError::InvalidEnum {
                        env_key: _EXO_UPSTREAM_ENDPOINT_URL,
                        env_value: actual_value.clone(),
                        message: format!("Must be set to a valid URL, got: {}", actual_value),
                    }
                })?;
                Ok(DeploymentMode::Playground(endpoint_url))
            }
            "production" => Ok(DeploymentMode::Production),
            other => Err(EnvError::InvalidEnum {
                env_key: EXO_ENV,
                env_value: other.to_string(),
                message: format!(
                    "Must be one of 'yolo', 'dev', 'test', 'playground', or 'production', got: {}",
                    other
                ),
            }),
        })
        .transpose()
}

pub fn load_env(mode: &DeploymentMode) -> impl Environment + Send + Sync + 'static {
    let mode_key = mode.env_key();

    // Files in order of precedence
    let env_files = [
        format!(".env.{}.local", mode_key),
        ".env.local".to_string(),
        format!(".env.{}", mode_key),
        ".env".to_string(),
    ];

    let mut envs: Vec<Arc<dyn Environment>> = vec![Arc::new(SystemEnvironment)];

    for env_file in env_files.iter() {
        envs.push(Arc::new(DotEnvironment::new(env_file)));
    }

    CompositeEnvironment::new(envs)
}

pub fn is_production(env: &dyn Environment) -> bool {
    matches!(
        get_deployment_mode(env),
        Ok(Some(DeploymentMode::Production))
    )
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
