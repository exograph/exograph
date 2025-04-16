use serde_json::Value;

use jsonwebtoken::{decode, DecodingKey, Validation};
use thiserror::Error;
use tracing::error;

use exo_env::Environment;

use crate::context::error::ContextExtractionError;
use crate::context::provider::cookie::CookieExtractor;
use crate::env_const::{
    EXO_JWT_SECRET, EXO_JWT_SOURCE_COOKIE, EXO_JWT_SOURCE_HEADER, EXO_OIDC_URL,
};
use crate::http::RequestHead;

use super::oidc::Oidc;

const TOKEN_PREFIX: &str = "Bearer ";

pub struct JwtAuthenticator {
    style: JwtAuthenticatorStyle,
    authenticator_source: AuthenticatorSource,
}

#[derive(Debug)]
enum AuthenticatorSource {
    Header(String),
    Cookie(String),
}

/// Authenticator with information about how to validate JWT tokens
/// It can be either a secret or a OIDC url
enum JwtAuthenticatorStyle {
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

        let style = match (secret, oidc_url) {
            (Some(secret), None) => Ok(JwtAuthenticatorStyle::Secret(secret)),
            (None, Some(oidc_url)) => Ok(JwtAuthenticatorStyle::Oidc(Oidc::new(oidc_url).await?)),
            (Some(_), Some(_)) => {
                Err(JwtConfigurationError::InvalidSetup(format!("Both {EXO_JWT_SECRET} and {EXO_OIDC_URL} are set. Only one of them can be set at a time")))
            }
            (None, None) => return Ok(None),
        }?;

        let jwt_source_header = env.get(EXO_JWT_SOURCE_HEADER);
        let jwt_source_cookie = env.get(EXO_JWT_SOURCE_COOKIE);

        match (jwt_source_header, jwt_source_cookie) {
            (Some(header), None) => Ok(Some(JwtAuthenticator {
                style,
                authenticator_source: AuthenticatorSource::Header(header),
            })),
            (None, Some(cookie)) => Ok(Some(JwtAuthenticator {
                style,
                authenticator_source: AuthenticatorSource::Cookie(cookie),
            })),
            (None, None) => Ok(Some(JwtAuthenticator {
                style,
                authenticator_source: AuthenticatorSource::Header("Authorization".to_string()),
            })),
            (Some(_), Some(_)) => {
                Err(JwtConfigurationError::InvalidSetup(
                    format!(
                        "Both {EXO_JWT_SOURCE_HEADER} and {EXO_JWT_SOURCE_COOKIE} are set. Only one of them can be set at a time"
                    )
                ))
            }
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

        match &self.style {
            JwtAuthenticatorStyle::Secret(secret) => Ok(decode::<Value>(
                token,
                &DecodingKey::from_secret(secret.as_ref()),
                &Validation::default(),
            )
            .map_err(map_jwt_error)?
            .claims),

            JwtAuthenticatorStyle::Oidc(oidc) => {
                oidc.validate(token).await.map_err(|err| match err {
                    oidc_jwt_validator::ValidationError::ValidationFailed(err) => {
                        map_jwt_error(err)
                    }
                    err => {
                        error!("Error validating JWT: {}", err);
                        JwtAuthenticationError::Invalid
                    }
                })
            }
        }
    }

    /// Extract authentication form the source (header or cookie) with a bearer token
    pub fn extract_jwt_token(
        &self,
        request_head: &(dyn RequestHead + Send + Sync),
    ) -> Result<Option<String>, ContextExtractionError> {
        match &self.authenticator_source {
            AuthenticatorSource::Header(header) => {
                if let Some(header) = request_head.get_header(header) {
                    if header.starts_with(TOKEN_PREFIX) {
                        Ok(Some(header[TOKEN_PREFIX.len()..].to_string()))
                    } else {
                        Err(ContextExtractionError::Malformed)
                    }
                } else {
                    Ok(None)
                }
            }
            AuthenticatorSource::Cookie(cookie) => {
                let mut cookies = CookieExtractor::extract_cookies(request_head)?;
                Ok(cookies.remove(cookie))
            }
        }
    }

    /// Extract and process the JWT token.
    ///
    /// The claim is deserialized into an opaque json `Value`, which will be eventually be mapped to
    /// the declared user context model
    pub(super) async fn extract_authentication(
        &self,
        request_head: &(dyn RequestHead + Send + Sync),
    ) -> Result<Value, ContextExtractionError> {
        let jwt_token = self.extract_jwt_token(request_head)?;

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
                // Either the source (header or cookie) was absent or the next token wasn't "Bearer"
                // It is not an error to have no authorization header, since that indicates an anonymous user
                // and there may be queries allowed for such users.
                Ok(serde_json::Value::Null)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        time::{SystemTime, UNIX_EPOCH},
    };

    use exo_env::MapEnvironment;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;

    use crate::{
        env_const::{EXO_JWT_SOURCE_COOKIE, EXO_JWT_SOURCE_HEADER},
        http::MemoryRequestHead,
    };

    use super::*;

    #[tokio::test]
    async fn invalid_style() {
        let env = MapEnvironment::from([(EXO_JWT_SECRET, "secret"), (EXO_OIDC_URL, "oidc")]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await;
        assert_eq!(authenticator.is_err(), true);
    }

    #[tokio::test]
    async fn invalid_source() {
        let env = MapEnvironment::from([
            (EXO_JWT_SECRET, "secret"),
            (EXO_JWT_SOURCE_HEADER, "jwt-header"),
            (EXO_JWT_SOURCE_COOKIE, "jwt-cookie"),
        ]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await;

        assert_eq!(authenticator.is_err(), true);
    }

    #[tokio::test]
    async fn no_token() {
        let env = MapEnvironment::from([(EXO_JWT_SECRET, "secret")]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let request_head = request_head_with_headers(HashMap::new());

        let authentication = authenticator.extract_authentication(&request_head).await;
        assert_eq!(authentication.unwrap(), serde_json::Value::Null);
    }

    #[tokio::test]
    async fn malformed_token() {
        let env = MapEnvironment::from([(EXO_JWT_SECRET, "secret")]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let claims = json!({
            "sub": "b@b.com",
            "company": "ACME",
        });

        let (token, _) = create_token(&claims, "secret", 100, TokenSource::Header);

        let malformed_token = token + "invalid";

        let request_head = request_head_with_headers(HashMap::from([(
            "Authorization".to_string(),
            vec![malformed_token],
        )]));

        let authentication = authenticator.extract_authentication(&request_head).await;
        assert_eq!(authentication.is_err(), true);
    }

    #[tokio::test]
    async fn expired_token() {
        let env = MapEnvironment::from([(EXO_JWT_SECRET, "secret")]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let claims = json!({
            "sub": "b@b.com",
            "company": "ACME",
        });

        let (token, _) = create_token(&claims, "secret", -100, TokenSource::Header);

        let request_head =
            request_head_with_headers(HashMap::from([("Authorization".to_string(), vec![token])]));

        let authentication = authenticator.extract_authentication(&request_head).await;
        assert_eq!(authentication.is_err(), true);
    }

    #[tokio::test]
    async fn valid_token_default_header() {
        let env = MapEnvironment::from([(EXO_JWT_SECRET, "secret")]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let claims = json!({
            "sub": "b@b.com",
            "company": "ACME",
        });

        let (token, claims) = create_token(&claims, "secret", 100, TokenSource::Header);

        let request_head =
            request_head_with_headers(HashMap::from([("Authorization".to_string(), vec![token])]));

        let authentication = authenticator.extract_authentication(&request_head).await;
        assert_eq!(authentication.unwrap(), claims);
    }

    #[tokio::test]
    async fn valid_token_custom_header() {
        let env = MapEnvironment::from([
            (EXO_JWT_SECRET, "secret"),
            (EXO_JWT_SOURCE_HEADER, "jwt-header"),
        ]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let claims = json!({
            "sub": "b@b.com",
            "company": "ACME",
        });

        let (token, claims) = create_token(&claims, "secret", 100, TokenSource::Header);

        let request_head =
            request_head_with_headers(HashMap::from([("jwt-header".to_string(), vec![token])]));

        let authentication = authenticator.extract_authentication(&request_head).await;
        assert_eq!(authentication.unwrap(), claims);
    }

    #[tokio::test]
    async fn valid_token_default_header_with_other_headers() {
        let env = MapEnvironment::from([(EXO_JWT_SECRET, "secret")]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let claims = json!({
            "sub": "b@b.com",
            "company": "ACME",
        });

        let (token, _) = create_token(&claims, "secret", 100, TokenSource::Header);

        for header_name in ["secret", "other-header"] {
            let request_head = request_head_with_headers(HashMap::from([(
                header_name.to_string(),
                vec![token.clone()],
            )]));

            let authentication = authenticator.extract_authentication(&request_head).await;
            assert_eq!(authentication.unwrap(), Value::Null);
        }
    }

    #[tokio::test]
    async fn valid_token_custom_header_with_other_headers() {
        let env = MapEnvironment::from([
            (EXO_JWT_SECRET, "secret"),
            (EXO_JWT_SOURCE_HEADER, "jwt-header"),
        ]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let claims = json!({
            "sub": "b@b.com",
            "company": "ACME",
        });

        let (token, _) = create_token(&claims, "secret", 100, TokenSource::Header);

        // The client passes the token in two different headers ("Authorization" is not the right header due to the custom header configuration)
        for header_name in ["Authorization", "other-header"] {
            let request_head = request_head_with_headers(HashMap::from([(
                header_name.to_string(),
                vec![token.clone()],
            )]));

            let authentication = authenticator.extract_authentication(&request_head).await;
            assert_eq!(authentication.unwrap(), Value::Null);
        }
    }

    #[tokio::test]
    async fn valid_token_custom_cookie() {
        let env = MapEnvironment::from([
            (EXO_JWT_SECRET, "secret"),
            (EXO_JWT_SOURCE_COOKIE, "jwt-cookie"),
        ]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let claims = json!({
            "sub": "b@b.com",
            "company": "ACME",
        });

        let (token, claims) = create_token(&claims, "secret", 100, TokenSource::Cookie);

        let request_head =
            request_head_with_cookies(HashMap::from([("jwt-cookie".to_string(), token)]));

        let authentication = authenticator.extract_authentication(&request_head).await;
        assert_eq!(authentication.unwrap(), claims);
    }

    #[tokio::test]
    async fn valid_token_custom_cookie_with_other_cookies() {
        let env = MapEnvironment::from([
            (EXO_JWT_SECRET, "secret"),
            (EXO_JWT_SOURCE_COOKIE, "jwt-cookie"),
        ]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let claims = json!({
            "sub": "b@b.com",
            "company": "ACME",
        });

        let (token, _) = create_token(&claims, "secret", 100, TokenSource::Cookie);

        // The client passes the token in two different cookies
        for cookie_name in ["Authorization", "other-cookie"] {
            let request_head = request_head_with_cookies(HashMap::from([(
                cookie_name.to_string(),
                token.clone(),
            )]));

            let authentication = authenticator.extract_authentication(&request_head).await;
            assert_eq!(authentication.unwrap(), Value::Null);
        }
    }

    fn request_head_with_headers(headers: HashMap<String, Vec<String>>) -> MemoryRequestHead {
        MemoryRequestHead::new(
            headers,
            HashMap::new(),
            http::Method::GET,
            "/".to_string(),
            Value::Null,
            None,
        )
    }

    fn request_head_with_cookies(cookies: HashMap<String, String>) -> MemoryRequestHead {
        MemoryRequestHead::new(
            HashMap::new(),
            cookies,
            http::Method::GET,
            "/".to_string(),
            Value::Null,
            None,
        )
    }

    enum TokenSource {
        Header,
        Cookie,
    }

    fn create_token(
        claims: &Value,
        secret: &str,
        expiration_seconds: i64,
        source: TokenSource,
    ) -> (String, Value) {
        let mut with_expiration = claims.clone().as_object().unwrap().clone();
        let current_epoch_time = {
            let start = SystemTime::now();
            let since_the_epoch = start
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards");
            since_the_epoch.as_secs()
        };
        with_expiration.insert(
            "exp".to_string(),
            json!(current_epoch_time as i64 + expiration_seconds),
        );

        let token = encode(
            &Header::default(),
            &with_expiration,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        (
            match source {
                TokenSource::Header => TOKEN_PREFIX.to_string() + &token,
                TokenSource::Cookie => token,
            },
            Value::Object(with_expiration),
        )
    }
}
