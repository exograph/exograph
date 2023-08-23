use async_recursion::async_recursion;
use serde_json::Value;
use tokio::sync::RwLock;

use thiserror::Error;

use jsonwebtoken::{
    decode, decode_header,
    jwk::{AlgorithmParameters, JwkSet},
    DecodingKey, TokenData, Validation,
};

#[derive(Debug, Error)]
pub enum JwksExtractionError {
    #[error("Failed to fetch JWKS `{0}`")]
    JwksLoadingError(#[from] reqwest::Error),

    #[error("Failed to parse JWKS `{0}`")]
    JwksParsingError(#[from] serde_json::Error),

    #[error("Token extraction error `{0}`")]
    JsonWebToken(#[from] jsonwebtoken::errors::Error),

    #[error("Token doesn't have a `kid` header field")]
    NoKid,

    #[error("Token doesn't have an `alg` header field")]
    NoAlgorithm,

    #[error("No matching kid `{0}`")]
    NoMatchingKid(String),
}

/// Information about a JWKS endpoint
///
/// The primary information is the URL. However, we cache the information (JWKs) to avoid
/// fetching and deserializing it for every request.
///
/// Uses a read-write lock to store the JWKS, since we expect many concurrent reads and
/// infrequent writes (only once, in fact, until we also take care of keys rotation).
pub struct JwksEndpoint {
    url: String,
    jwks: RwLock<Option<JwkSet>>,
}

impl JwksEndpoint {
    pub(super) fn new(url: String) -> Self {
        Self {
            url,
            jwks: RwLock::new(None),
        }
    }

    #[async_recursion]
    pub(super) async fn decode_token(
        &self,
        token: &str,
    ) -> Result<TokenData<Value>, JwksExtractionError> {
        let reader = self.jwks.read().await;

        match &*reader {
            Some(jwks) => {
                // Based on https://github.com/Keats/jsonwebtoken/blob/master/examples/auth0.rs
                let header = decode_header(token)?;

                let kid = header.kid.ok_or(JwksExtractionError::NoKid)?;

                if let Some(jwk) = jwks.find(&kid) {
                    match &jwk.algorithm {
                        AlgorithmParameters::RSA(rsa) => {
                            let decoding_key = DecodingKey::from_rsa_components(&rsa.n, &rsa.e)?;
                            let validation = jwk
                                .common
                                .algorithm
                                .map(Validation::new)
                                .ok_or(JwksExtractionError::NoAlgorithm)?;

                            Ok(decode::<Value>(token, &decoding_key, &validation)?)
                        }
                        _ => unreachable!("this should be a RSA"),
                    }
                } else {
                    Err(JwksExtractionError::NoMatchingKid(kid))
                }
            }
            None => {
                // If we don't have the JWKS, we need to fetch it, but first drop the read lock, so
                // that we can acquire the write lock.
                drop(reader);

                let mut writer = self.jwks.write().await;

                if writer.is_none() {
                    *writer = {
                        let response = reqwest::get(&self.url).await?;
                        let jwks_reply = response.bytes().await?;
                        Some(serde_json::from_slice(jwks_reply.as_ref())?)
                    }
                }

                // Now that we have the JWKS, we can drop the write lock so that the recursive call
                // can acquire the read lock.
                drop(writer);

                self.decode_token(token).await
            }
        }
    }
}
