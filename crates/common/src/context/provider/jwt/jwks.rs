use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, warn};

use super::authenticator::{JwtConfigurationError, jwt_debug_enabled, jwt_debug_log};

#[derive(Debug, Serialize, Deserialize)]
struct Jwks {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JwkKey {
    #[serde(rename = "kty")]
    key_type: String,
    #[serde(rename = "use")]
    key_use: Option<String>,
    kid: Option<String>,
    alg: Option<String>,
    n: String,
    e: String,
}

pub struct JwksValidator {
    jwks_url: String,
    keys: HashMap<String, DecodingKey>,
    client: reqwest::Client,
    allowed_audiences: Option<Vec<String>>,
}

impl JwksValidator {
    pub async fn new(jwks_url: String) -> Result<Self, JwtConfigurationError> {
        Self::new_with_audiences(jwks_url, None).await
    }

    pub async fn new_with_audiences(
        jwks_url: String,
        allowed_audiences: Option<Vec<String>>,
    ) -> Result<Self, JwtConfigurationError> {
        let client = reqwest::ClientBuilder::new().build().map_err(|e| {
            JwtConfigurationError::Configuration {
                message: "Unable to create HTTP client".to_owned(),
                source: e.into(),
            }
        })?;

        let mut validator = Self {
            jwks_url: jwks_url.clone(),
            keys: HashMap::new(),
            client: client.clone(),
            allowed_audiences,
        };

        // Fetch initial keys
        validator.refresh_keys().await?;

        jwt_debug_log(|| {
            let kids: Vec<String> = validator.keys.keys().cloned().collect();
            format!(
                "Initialized JWKS provider '{}' with {} key(s); kids={:?}; audience_filter={:?}",
                jwks_url,
                kids.len(),
                kids,
                validator.allowed_audiences.as_ref()
            )
        });

        Ok(validator)
    }

    async fn refresh_keys(&mut self) -> Result<(), JwtConfigurationError> {
        let response = self.client.get(&self.jwks_url).send().await.map_err(|e| {
            JwtConfigurationError::Configuration {
                message: format!("Failed to fetch JWKS from {}", self.jwks_url),
                source: e.into(),
            }
        })?;

        let jwks: Jwks =
            response
                .json()
                .await
                .map_err(|e| JwtConfigurationError::Configuration {
                    message: "Failed to parse JWKS response".to_owned(),
                    source: e.into(),
                })?;

        let mut new_keys = HashMap::new();
        for key in jwks.keys {
            if key.key_type != "RSA" {
                warn!("Skipping non-RSA key: {:?}", key.kid);
                continue;
            }

            let kid = key.kid.unwrap_or_else(|| "default".to_string());

            match DecodingKey::from_rsa_components(&key.n, &key.e) {
                Ok(decoding_key) => {
                    new_keys.insert(kid.clone(), decoding_key);
                }
                Err(e) => {
                    error!("Failed to create decoding key for kid {}: {}", kid, e);
                }
            }
        }

        if new_keys.is_empty() {
            return Err(JwtConfigurationError::Configuration {
                message: format!("No valid RSA keys found in JWKS from {}", self.jwks_url),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No valid keys",
                )),
            });
        }

        self.keys = new_keys;
        Ok(())
    }

    pub async fn validate(&self, token: &str) -> Result<Value, JwtValidationError> {
        // Decode header to get kid
        let header = decode_header(token).map_err(|e| {
            error!("Failed to decode JWT header: {}", e);
            JwtValidationError::Invalid
        })?;

        let kid = header.kid.unwrap_or_else(|| "default".to_string());

        // Get the decoding key for this kid
        let decoding_key = self.keys.get(&kid).ok_or_else(|| {
            error!("No key found for kid: {}", kid);
            if jwt_debug_enabled() {
                let available: Vec<&String> = self.keys.keys().collect();
                eprintln!(
                    "[JWT Debug] JWKS '{}' does not contain kid '{}'. Available kids: {:?}",
                    self.jwks_url, kid, available
                );
            }
            JwtValidationError::Invalid
        })?;

        // Create validation settings
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        validation.validate_nbf = false;

        // Configure audience validation based on settings
        if let Some(audiences) = &self.allowed_audiences {
            validation.set_audience(audiences);
            validation.validate_aud = true;
        } else {
            // If no audiences configured, skip audience validation
            validation.validate_aud = false;
        }

        validation.set_required_spec_claims::<&str>(&[]); // Don't require iss claim

        // Decode and validate token
        eprintln!("[JWKS Validator] Attempting to validate with kid: {}", kid);
        eprintln!(
            "[JWKS Validator] Validation settings: exp={}, nbf={}, aud={}",
            validation.validate_exp, validation.validate_nbf, validation.validate_aud
        );

        let token_data = decode::<Value>(token, decoding_key, &validation).map_err(|e| {
            eprintln!("[JWKS Validator] Validation failed: {:?}", e);
            match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    error!("JWT validation failed: expired signature");
                    if jwt_debug_enabled() {
                        eprintln!(
                            "[JWT Debug] JWKS '{}' reports expired token while validating kid '{}'",
                            self.jwks_url, kid
                        );
                    }
                    JwtValidationError::Expired
                }
                _ => {
                    error!("JWT validation failed: {}", e);
                    if jwt_debug_enabled() {
                        eprintln!(
                            "[JWT Debug] JWKS '{}' failed to validate kid '{}': {}",
                            self.jwks_url, kid, e
                        );
                    }
                    JwtValidationError::Invalid
                }
            }
        })?;

        eprintln!("[JWKS Validator] Validation successful!");

        Ok(token_data.claims)
    }
}

impl JwksValidator {
    pub fn debug_source(&self) -> &str {
        &self.jwks_url
    }

    pub fn debug_known_kids(&self) -> Vec<String> {
        self.keys.keys().cloned().collect()
    }

    pub fn debug_allowed_audiences(&self) -> Option<&[String]> {
        self.allowed_audiences.as_deref()
    }
}

#[derive(Debug)]
pub enum JwtValidationError {
    Invalid,
    Expired,
    KidMismatch,
}

impl std::fmt::Display for JwtValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtValidationError::Invalid => write!(f, "Invalid token"),
            JwtValidationError::Expired => write!(f, "Expired token"),
            JwtValidationError::KidMismatch => write!(f, "Token kid does not match"),
        }
    }
}

impl std::error::Error for JwtValidationError {}
