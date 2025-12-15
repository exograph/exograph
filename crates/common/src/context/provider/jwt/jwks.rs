use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, warn};

use super::authenticator::JwtConfigurationError;

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
}

impl JwksValidator {
    pub async fn new(jwks_url: String) -> Result<Self, JwtConfigurationError> {
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
        };

        // Fetch initial keys
        validator.refresh_keys().await?;

        Ok(validator)
    }

    async fn refresh_keys(&mut self) -> Result<(), JwtConfigurationError> {
        let response = self
            .client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| JwtConfigurationError::Configuration {
                message: format!("Failed to fetch JWKS from {}", self.jwks_url),
                source: e.into(),
            })?;

        let jwks: Jwks = response
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
            JwtValidationError::Invalid
        })?;

        // Create validation settings
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        validation.validate_nbf = false;
        
        // Don't validate issuer/audience - let Hasura handle that in claims
        validation.validate_aud = false;

        // Decode and validate token
        let token_data = decode::<Value>(token, decoding_key, &validation).map_err(|e| {
            match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    JwtValidationError::Expired
                }
                _ => {
                    error!("JWT validation failed: {}", e);
                    JwtValidationError::Invalid
                }
            }
        })?;

        Ok(token_data.claims)
    }
}

#[derive(Debug)]
pub enum JwtValidationError {
    Invalid,
    Expired,
}

impl std::fmt::Display for JwtValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtValidationError::Invalid => write!(f, "Invalid token"),
            JwtValidationError::Expired => write!(f, "Expired token"),
        }
    }
}

impl std::error::Error for JwtValidationError {}
