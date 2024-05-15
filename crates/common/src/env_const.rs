use super::EnvError;

pub const EXO_INTROSPECTION: &str = "EXO_INTROSPECTION";
pub const EXO_INTROSPECTION_LIVE_UPDATE: &str = "EXO_INTROSPECTION_LIVE_UPDATE";

pub const EXO_CORS_DOMAINS: &str = "EXO_CORS_DOMAINS";

pub const EXO_JWT_SECRET: &str = "EXO_JWT_SECRET";
pub const EXO_OIDC_URL: &str = "EXO_OIDC_URL";

pub const EXO_POSTGRES_URL: &str = "EXO_POSTGRES_URL";
pub const DATABASE_URL: &str = "DATABASE_URL";
pub const EXO_CONNECTION_POOL_SIZE: &str = "EXO_CONNECTION_POOL_SIZE";
pub const EXO_CHECK_CONNECTION_ON_STARTUP: &str = "EXO_CHECK_CONNECTION_ON_STARTUP";

pub const EXO_SERVER_PORT: &str = "EXO_SERVER_PORT";

pub const _EXO_DEPLOYMENT_MODE: &str = "_EXO_DEPLOYMENT_MODE"; // "yolo", "dev", "playground" or "prod" (default)
pub const _EXO_ENFORCE_TRUSTED_DOCUMENTS: &str = "_EXO_ENFORCE_TRUSTED_DOCUMENTS";

pub const _EXO_UPSTREAM_ENDPOINT_URL: &str = "_EXO_UPSTREAM_ENDPOINT_URL";

#[derive(Debug)]
pub enum DeploymentMode {
    Yolo,
    Dev,
    Playground(String), // URL of the GraphQL endpoint to connect to
    Prod,
}

pub fn get_deployment_mode() -> Result<DeploymentMode, EnvError> {
    match std::env::var(_EXO_DEPLOYMENT_MODE).as_deref() {
        Ok("yolo") => Ok(DeploymentMode::Yolo),
        Ok("dev") => Ok(DeploymentMode::Dev),
        Ok("playground") => {
            let endpoint_url =
                std::env::var(_EXO_UPSTREAM_ENDPOINT_URL).map_err(|_| EnvError::InvalidEnum {
                    env_key: _EXO_UPSTREAM_ENDPOINT_URL,
                    env_value: "".to_string(),
                    message: "Must be set to a valid URL".to_string(),
                })?;
            Ok(DeploymentMode::Playground(endpoint_url))
        }
        Ok("prod") | Err(_) => Ok(DeploymentMode::Prod),
        Ok(other) => Err(EnvError::InvalidEnum {
            env_key: _EXO_DEPLOYMENT_MODE,
            env_value: other.to_string(),
            message: "Must be one of 'yolo', 'dev', 'playground', or 'prod'".to_string(),
        }),
    }
}

pub fn is_production() -> bool {
    matches!(get_deployment_mode(), Ok(DeploymentMode::Prod) | Err(_))
}

pub fn get_enforce_trusted_documents() -> bool {
    std::env::var(_EXO_ENFORCE_TRUSTED_DOCUMENTS)
        .map(|value| value != "false")
        .unwrap_or(true)
}
