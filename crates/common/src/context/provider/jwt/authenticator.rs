use serde_json::Value;

use jsonwebtoken::{DecodingKey, Validation, decode};
use thiserror::Error;
use tracing::error;

use exo_env::Environment;

use crate::context::error::ContextExtractionError;
use crate::context::provider::cookie::CookieExtractor;
use crate::env_const::{
    EXO_JWT_SECRET, EXO_JWT_SOURCE_COOKIE, EXO_JWT_SOURCE_HEADER, EXO_OIDC_URL, EXO_OIDC_URLS, EXO_JWKS_URLS,
};
use crate::http::RequestHead;

use super::oidc::Oidc;
use super::jwks::JwksValidator;

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
/// It can be either a secret, OIDC url(s), JWKS url(s), or a mix of OIDC and JWKS
enum JwtAuthenticatorStyle {
    Secret(String),
    Oidc(Oidc),
    MultiOidc(Vec<Oidc>),
    Jwks(JwksValidator),
    MultiJwks(Vec<JwksValidator>),
    Mixed {
        oidc: Vec<Oidc>,
        jwks: Vec<JwksValidator>,
    },
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
        let oidc_urls = env.get(EXO_OIDC_URLS);
        let jwks_urls = env.get(EXO_JWKS_URLS);

        let style = match (secret, oidc_url, oidc_urls, jwks_urls) {
            (Some(secret), None, None, None) => Ok(JwtAuthenticatorStyle::Secret(secret)),
            (None, Some(oidc_url), None, None) => Ok(JwtAuthenticatorStyle::Oidc(Oidc::new(oidc_url).await?)),
            (None, None, Some(oidc_urls_str), None) => {
                // Parse comma-separated OIDC URLs
                let urls: Vec<String> = oidc_urls_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                if urls.is_empty() {
                    return Err(JwtConfigurationError::InvalidSetup(
                        format!("{EXO_OIDC_URLS} is set but contains no valid URLs")
                    ));
                }
                
                // Initialize all OIDC validators
                let mut oidc_validators = Vec::new();
                for (idx, url) in urls.iter().enumerate() {
                    match Oidc::new(url.clone()).await {
                        Ok(validator) => {
                            tracing::info!("Initialized OIDC provider {}: {}", idx + 1, url);
                            oidc_validators.push(validator);
                        }
                        Err(e) => {
                            return Err(JwtConfigurationError::Configuration {
                                message: format!("Failed to initialize OIDC provider '{}': {}", url, e),
                                source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
                            });
                        }
                    }
                }
                
                Ok(JwtAuthenticatorStyle::MultiOidc(oidc_validators))
            },
            (None, None, None, Some(jwks_urls_str)) => {
                // Parse comma-separated JWKS URLs (for Hasura/Nhost)
                let urls: Vec<String> = jwks_urls_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                if urls.is_empty() {
                    return Err(JwtConfigurationError::InvalidSetup(
                        format!("{EXO_JWKS_URLS} is set but contains no valid URLs")
                    ));
                }
                
                // Initialize all JWKS validators
                let mut jwks_validators = Vec::new();
                for (idx, url) in urls.iter().enumerate() {
                    match JwksValidator::new(url.clone()).await {
                        Ok(validator) => {
                            tracing::info!("Initialized JWKS provider {}: {}", idx + 1, url);
                            jwks_validators.push(validator);
                        }
                        Err(e) => {
                            return Err(JwtConfigurationError::Configuration {
                                message: format!("Failed to initialize JWKS provider '{}': {}", url, e),
                                source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
                            });
                        }
                    }
                }
                
                Ok(JwtAuthenticatorStyle::MultiJwks(jwks_validators))
            },
            (Some(_), _, _, _) => Err(JwtConfigurationError::InvalidSetup(format!(
                "{EXO_JWT_SECRET} cannot be used with any other JWT configuration"
            ))),
            (None, Some(_), Some(_), _) => Err(JwtConfigurationError::InvalidSetup(format!(
                "Both {EXO_OIDC_URL} and {EXO_OIDC_URLS} are set. Use only {EXO_OIDC_URLS} for multiple providers"
            ))),
            (None, _, Some(oidc_urls_str), Some(jwks_urls_str)) => {
                // Mixed mode: both OIDC and JWKS providers
                let oidc_urls: Vec<String> = oidc_urls_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                let jwks_urls: Vec<String> = jwks_urls_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                if oidc_urls.is_empty() && jwks_urls.is_empty() {
                    return Err(JwtConfigurationError::InvalidSetup(
                        "Both EXO_OIDC_URLS and EXO_JWKS_URLS are set but empty".to_string()
                    ));
                }
                
                let mut oidc_validators = Vec::new();
                for (idx, url) in oidc_urls.iter().enumerate() {
                    match Oidc::new(url.clone()).await {
                        Ok(validator) => {
                            tracing::info!("Initialized OIDC provider {}: {}", idx + 1, url);
                            oidc_validators.push(validator);
                        }
                        Err(e) => {
                            return Err(JwtConfigurationError::Configuration {
                                message: format!("Failed to initialize OIDC provider '{}': {}", url, e),
                                source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
                            });
                        }
                    }
                }
                
                let mut jwks_validators = Vec::new();
                for (idx, url) in jwks_urls.iter().enumerate() {
                    match JwksValidator::new(url.clone()).await {
                        Ok(validator) => {
                            tracing::info!("Initialized JWKS provider {}: {}", idx + 1, url);
                            jwks_validators.push(validator);
                        }
                        Err(e) => {
                            return Err(JwtConfigurationError::Configuration {
                                message: format!("Failed to initialize JWKS provider '{}': {}", url, e),
                                source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e))),
                            });
                        }
                    }
                }
                
                Ok(JwtAuthenticatorStyle::Mixed {
                    oidc: oidc_validators,
                    jwks: jwks_validators,
                })
            },
            (None, Some(_), _, Some(_)) => Err(JwtConfigurationError::InvalidSetup(format!(
                "Both {EXO_OIDC_URL} and {EXO_JWKS_URLS} are set. Use {EXO_OIDC_URLS} for multiple OIDC providers"
            ))),
            (None, None, None, None) => return Ok(None),
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
            (Some(_), Some(_)) => Err(JwtConfigurationError::InvalidSetup(format!(
                "Both {EXO_JWT_SOURCE_HEADER} and {EXO_JWT_SOURCE_COOKIE} are set. Only one of them can be set at a time"
            ))),
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

            JwtAuthenticatorStyle::MultiOidc(oidc_validators) => {
                // Try each OIDC provider in sequence until one succeeds
                let mut last_error = None;
                
                for oidc in oidc_validators {
                    match oidc.validate(token).await {
                        Ok(claims) => return Ok(claims),
                        Err(err) => {
                            // Store the last error for reporting if all fail
                            last_error = Some(err);
                        }
                    }
                }
                
                // All validators failed, return the last error
                Err(match last_error {
                    Some(oidc_jwt_validator::ValidationError::ValidationFailed(err)) => {
                        map_jwt_error(err)
                    }
                    Some(err) => {
                        error!("Error validating JWT with all providers: {}", err);
                        JwtAuthenticationError::Invalid
                    }
                    None => JwtAuthenticationError::Invalid,
                })
            }

            JwtAuthenticatorStyle::Jwks(jwks) => {
                jwks.validate(token).await.map_err(|err| match err {
                    super::jwks::JwtValidationError::Expired => JwtAuthenticationError::Expired,
                    super::jwks::JwtValidationError::Invalid => JwtAuthenticationError::Invalid,
                })
            }

            JwtAuthenticatorStyle::MultiJwks(jwks_validators) => {
                // Try each JWKS provider in sequence until one succeeds
                let mut last_error = None;
                
                for jwks in jwks_validators {
                    match jwks.validate(token).await {
                        Ok(claims) => return Ok(claims),
                        Err(err) => {
                            // Store the last error for reporting if all fail
                            last_error = Some(err);
                        }
                    }
                }
                
                // All validators failed, return the last error
                Err(match last_error {
                    Some(super::jwks::JwtValidationError::Expired) => JwtAuthenticationError::Expired,
                    Some(super::jwks::JwtValidationError::Invalid) => {
                        error!("Error validating JWT with all JWKS providers");
                        JwtAuthenticationError::Invalid
                    }
                    None => JwtAuthenticationError::Invalid,
                })
            }
            
            JwtAuthenticatorStyle::Mixed { oidc, jwks } => {
                // Try OIDC providers first
                for oidc_validator in oidc {
                    match oidc_validator.validate(token).await {
                        Ok(claims) => return Ok(claims),
                        Err(_) => {} // Continue to next provider
                    }
                }
                
                // Try JWKS providers
                for jwks_validator in jwks {
                    match jwks_validator.validate(token).await {
                        Ok(claims) => return Ok(claims),
                        Err(_) => {} // Continue to next provider
                    }
                }
                
                // All validators failed
                error!("Error validating JWT with all providers (OIDC + JWKS)");
                Err(JwtAuthenticationError::Invalid)
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
    use jsonwebtoken::{EncodingKey, Header, encode};
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
