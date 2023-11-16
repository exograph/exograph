use thiserror::Error;

pub const EXO_INTROSPECTION: &str = "EXO_INTROSPECTION";
pub const EXO_INTROSPECTION_LIVE_UPDATE: &str = "EXO_INTROSPECTION_LIVE_UPDATE";

pub const EXO_CORS_DOMAINS: &str = "EXO_CORS_DOMAINS";

pub const EXO_JWT_SECRET: &str = "EXO_JWT_SECRET";
pub const EXO_OIDC_URL: &str = "EXO_OIDC_URL";

pub const EXO_POSTGRES_URL: &str = "EXO_POSTGRES_URL";
pub const EXO_POSTGRES_USER: &str = "EXO_POSTGRES_USER";
pub const EXO_POSTGRES_PASSWORD: &str = "EXO_POSTGRES_PASSWORD";
pub const EXO_CONNECTION_POOL_SIZE: &str = "EXO_CONNECTION_POOL_SIZE";
pub const EXO_CHECK_CONNECTION_ON_STARTUP: &str = "EXO_CHECK_CONNECTION_ON_STARTUP";

pub const EXO_SERVER_PORT: &str = "EXO_SERVER_PORT";

pub const _EXO_DEPLOYMENT_MODE: &str = "_EXO_DEPLOYMENT_MODE"; // "yolo", "dev", "playground" or "prod" (default)

#[derive(Error, Debug)]
pub enum EnvError {
    #[error("Invalid env value {env_value} for {env_key}: {message}")]
    InvalidEnum {
        env_key: &'static str,
        env_value: String,
        message: String,
    },
}

pub enum DeploymentMode {
    Yolo,
    Dev,
    Playground,
    Prod,
}

pub fn get_deployment_mode() -> Result<DeploymentMode, EnvError> {
    match std::env::var(_EXO_DEPLOYMENT_MODE).as_deref() {
        Ok("yolo") => Ok(DeploymentMode::Yolo),
        Ok("dev") => Ok(DeploymentMode::Dev),
        Ok("playground") => Ok(DeploymentMode::Playground),
        Ok("prod") | Err(_) => Ok(DeploymentMode::Prod),
        Ok(other) => Err(EnvError::InvalidEnum {
            env_key: _EXO_DEPLOYMENT_MODE,
            env_value: other.to_string(),
            message: "Must be one of 'yolo', 'dev', 'playground', or 'prod'".to_string(),
        }),
    }
}
