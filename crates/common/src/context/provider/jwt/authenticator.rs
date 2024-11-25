use serde_json::Value;

use jsonwebtoken::{decode, DecodingKey, Validation};
use thiserror::Error;
use tracing::error;

use exo_env::Environment;

use crate::context::error::ContextExtractionError;
use crate::env_const::EXO_JWT_SECRET;
use crate::env_const::EXO_OIDC_URL;
use crate::http::RequestHead;

use super::oidc::Oidc;

/// Authenticator with information about how to validate JWT tokens
/// It can be either a secret or a OIDC url
pub enum JwtAuthenticator {
    Secret(String),
    Oidc(Oidc),
}

#[derive(Debug, Error)]
pub(super) enum JwtAuthenticationError {
    #[error("Invalid token")]
    Invalid,
    #[error("Expired token")]
    Expired,
    #[error("Delegate error: {0}")]
    Delegate(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
pub enum JwtConfigurationError {
    #[error("Invalid setup: {0}")]
    InvalidSetup(String),

    #[error("JWT configuration error '{message}'")]
    Configuration {
        message: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl JwtAuthenticator {
    pub async fn new_from_env(
        env: &dyn Environment,
    ) -> Result<Option<Self>, JwtConfigurationError> {
        let secret = env.get(EXO_JWT_SECRET);

        let oidc_url = env.get(EXO_OIDC_URL);

        match (secret, oidc_url) {
            (Some(secret), None) => Ok(Some(JwtAuthenticator::Secret(secret))),
            (None, Some(oidc_url)) => Ok(Some(JwtAuthenticator::Oidc(Oidc::new(oidc_url).await?))),
            (Some(_), Some(_)) => {
                Err(JwtConfigurationError::InvalidSetup(format!("Both {EXO_JWT_SECRET} and {EXO_OIDC_URL} are set. Only one of them can be set at a time")))
            }
            (None, None) => Ok(None),
        }
    }

    async fn validate_jwt(&self, token: &str) -> Result<Value, JwtAuthenticationError> {
        fn map_jwt_error(error: jsonwebtoken::errors::Error) -> JwtAuthenticationError {
            match error.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    JwtAuthenticationError::Expired
                }
                _ => JwtAuthenticationError::Invalid,
            }
        }

        match self {
            JwtAuthenticator::Secret(secret) => Ok(decode::<Value>(
                token,
                &DecodingKey::from_secret(secret.as_ref()),
                &Validation::default(),
            )
            .map_err(map_jwt_error)?
            .claims),
            JwtAuthenticator::Oidc(oidc) => oidc.validate(token).await.map_err(|err| match err {
                oidc_jwt_validator::ValidationError::ValidationFailed(err) => map_jwt_error(err),
                err => {
                    error!("Error validating JWT: {}", err);
                    JwtAuthenticationError::Invalid
                }
            }),
        }
    }

    /// Extract authentication form the "Authorization" header with a bearer token
    /// The claim is deserialized into an opaque json `Value`, which will be eventually be mapped to
    /// the declared user context model
    pub(super) async fn extract_authentication(
        &self,
        request_head: &(dyn RequestHead + Send + Sync),
    ) -> Result<Value, ContextExtractionError> {
        let jwt_token = request_head
            .get_header("Authorization")
            .and_then(|auth_token| auth_token.strip_prefix("Bearer ").map(|t| t.to_owned()));

        match jwt_token {
            Some(jwt_token) => self
                .validate_jwt(&jwt_token)
                .await
                .map_err(|err| match &err {
                    JwtAuthenticationError::Invalid => ContextExtractionError::Unauthorized,
                    JwtAuthenticationError::Expired => {
                        ContextExtractionError::ExpiredAuthentication
                    }
                    JwtAuthenticationError::Delegate(err) => {
                        error!("Error validating JWT: {}", err);
                        ContextExtractionError::Unauthorized
                    }
                }),
            None => {
                // Either the "Authorization" header was absent or the next token wasn't "Bearer"
                // It is not an error to have no authorization header, since that indicates an anonymous user
                // and there may be queries allowed for such users.
                Ok(serde_json::Value::Null)
            }
        }
    }
}
