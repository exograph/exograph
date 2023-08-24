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
/// The primary information is the URL (pointing to a .json, such as
/// https://<someting>.auth0.com/.well-known/jwks.json). However, we cache the information (JWKS) to
/// avoid fetching and deserializing it for every request.
///
/// Uses a read-write lock to store the JWKS, since we expect many concurrent reads and infrequent
/// writes (only once, in fact, until we also take care of key rotation).
pub struct JwksEndpoint {
    url: String,
    jwks: RwLock<Option<JwksData>>,
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
            Some(jwks) => jwks.decode_token(token).await,
            None => {
                // If we don't have the JWKS, we need to fetch it, but first drop the read lock, so
                // that we can acquire the write lock.
                drop(reader);

                let mut writer = self.jwks.write().await;

                // Check once again for `is_none()`, in case, another write had its turn between dropping the reader and acquiring the writer.
                if writer.is_none() {
                    *writer = {
                        let response = reqwest::get(&self.url).await?;
                        let jwks_reply = response.bytes().await?;
                        Some(JwksData::from_bytes(jwks_reply.as_ref())?)
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

struct JwksData {
    jwks: JwkSet,
}

impl JwksData {
    fn new(jwks: JwkSet) -> Self {
        Self { jwks }
    }

    fn from_bytes(jwks: &[u8]) -> Result<Self, serde_json::Error> {
        Ok(Self::new(serde_json::from_slice(jwks)?))
    }

    pub(super) async fn decode_token(
        &self,
        token: &str,
    ) -> Result<TokenData<Value>, JwksExtractionError> {
        // Based on https://github.com/Keats/jsonwebtoken/blob/master/examples/auth0.rs
        let header = decode_header(token)?;

        let kid = header.kid.ok_or(JwksExtractionError::NoKid)?;

        if let Some(jwk) = self.jwks.find(&kid) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::errors::ErrorKind;

    const JWKS_DATA: &str = r#"
        {
            "keys":[
                {
                    "alg":"RS256",
                    "e":"AQAB",
                    "key_ops":null,
                    "kid":"iUWzks1qXnk6EAsUEx2_6zlNqfV5czGMHBk9DV-i4W8",
                    "kty":"RSA",
                    "n":"te5ER3Q45KolFCmslVjjiCzvzREi3u_oRZhmcSgqqXlnISkQTahxSw77t0HP1eKFq2TOq0RpPG3m6kZrsieSWBq7yagzNnw0ShOXjrvDfh5J5zLCJQXwlJYyXT7pj_c1OCmSmopPdrRVUxSH4lq78xicor4hSEGu52mSPsa8jTaFUY9LjvHOqlf35q-Xdb7yNE1LWTQrrQkKtqG_ZnzRNThA23RGhLJu204YL8FL7TA8cs_zPNvBmUivOkEKr9VuTGJESdBnSNTKlruMJf_SlslCdfyr1FyqzTXObVO9vAF3gZXAjoAqOTkx9jLiekQy_ul5r3CgLmyGBXtSJ30_eQ",
                    "use":"sig"
                }
            ]
        }
    "#;

    #[tokio::test]
    async fn valid() {
        let jwks_data = JwksData::from_bytes(JWKS_DATA.as_bytes()).unwrap();
        // Key with "sub" set to 1234 and "role" set to "admin"
        let test_token = "eyJhbGciOiJSUzI1NiIsImtpZCI6ImlVV3prczFxWG5rNkVBc1VFeDJfNnpsTnFmVjVjekdNSEJrOURWLWk0VzgiLCJ0eXAiOiJKV1QifQ.eyJleHAiOjE5MTYyMzkwMjIsInJvbGUiOiJhZG1pbiIsInN1YiI6IjEyMzQifQ.UkiaHBXg_hqMjIxqbuhCjZIzWvi1SlLRs1UtHinmiGv8K2A33-0qSgKRser6SG2-3i0z3oKKZDfHw9Ye0r6yoaM-cCZFO8Rw3Gax75Ojl7FtJ6_Egpz0_TaCgH08SDwAMKFg2jpvlCNdyHwn2RM0tyL5vjudMzlqltDVxH0DWa2N7Oce5vZ07IMDBzbJUphCTwY6rPLlXFiJdneYOrmKbR19Wu9914B56w_KdhBvxb6PacjEwNtAkWmtHHOVVf8Sgox9AH0R5WG1UcGSuv1XLrU-qyYUDhXx0NgDWFDFrQJKtnTJ4p6-6CZpbcGAb_aZSEruZ1-bbKvWdNGbayE8Mg";

        let claims = jwks_data.decode_token(test_token).await.unwrap().claims;

        assert_eq!(claims["sub"], "1234");
        assert_eq!(claims["role"], "admin");
    }

    #[tokio::test]
    async fn expired() {
        let jwks_data = JwksData::from_bytes(JWKS_DATA.as_bytes()).unwrap();
        // A token that expired in 2018
        let test_token = "eyJhbGciOiJSUzI1NiIsImtpZCI6ImlVV3prczFxWG5rNkVBc1VFeDJfNnpsTnFmVjVjekdNSEJrOURWLWk0VzgiLCJ0eXAiOiJKV1QifQ.eyJleHAiOjE1MTYyMzkwMjIsInJvbGUiOiJhZG1pbiIsInN1YiI6IjEyMzQifQ.CWFJTKkx3ZYrAVI7eu1YjFzZeRTHSUBpPuaL0bkD_8eWbmNyhKRTR_Jrd5mjqWje7-2k8CgPxLh-N_wUei1vR5jUF5hDCV27T3L2SHLjARYS5_gfwBCMk-q0Z8aniZx_Mgmkqa3GcSMX9Z9_kLMNq45HMBZg3VAM2DSPeTBcGu9neMXMUVIliE__6LwMU_KTHZ_v7fNno6i9Qga0wh6k4TEDUaLwGrbBcbY2Ie7lcZ04dO-iIchj73NQ8FXxvj4YvUI1U-dmTQQNW0v5MAwZ1OXX8Z0WGqVIO2jiCrOV9D0pRjfFGaYBMLClGiWgYF2UUoAZWua1MPPquP2r8cYWkg";

        let token_data = jwks_data.decode_token(test_token).await;

        match token_data {
            Err(JwksExtractionError::JsonWebToken(err)) => {
                assert!(matches!(err.kind(), &ErrorKind::ExpiredSignature));
            }
            _ => panic!("expected expired signature error"),
        }
    }
}
