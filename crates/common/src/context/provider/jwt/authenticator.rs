use serde_json::Value;

use jsonwebtoken::{DecodingKey, Validation, decode};
use thiserror::Error;
use tracing::error;

use exo_env::Environment;

use crate::context::error::ContextExtractionError;
use crate::context::provider::cookie::CookieExtractor;
use crate::env_const::{
    EXO_JWKS_URLS, EXO_JWT_AUDIENCES, EXO_JWT_PUBLIC_KEY_KID, EXO_JWT_PUBLIC_KEY_PEM,
    EXO_JWT_PUBLIC_KEY_PEM_ENVS, EXO_JWT_SECRET, EXO_JWT_SOURCE_COOKIE, EXO_JWT_SOURCE_HEADER,
    EXO_OIDC_URL, EXO_OIDC_URLS,
};
use crate::http::RequestHead;

use super::jwks::{JwksValidator, JwtValidationError};
use super::oidc::Oidc;
use super::static_key::StaticKeyValidator;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::sync::OnceLock;

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
/// It can be either a secret, OIDC url(s), JWKS url(s), static public key(s), or a mix of the above
enum JwtAuthenticatorStyle {
    Secret(String),
    Oidc(Vec<Oidc>),
    Jwks(Vec<JwksValidator>),
    StaticKeys(Vec<StaticKeyValidator>),
    Mixed {
        oidc: Vec<Oidc>,
        jwks: Vec<JwksValidator>,
        static_keys: Vec<StaticKeyValidator>,
    },
}

enum ValidationOutcome {
    Success(Value),
    Expired,
    NoMatch,
}

struct ValidationAttempt {
    outcome: ValidationOutcome,
    had_non_kid_error: bool,
}

static JWT_DEBUG_FLAG: OnceLock<bool> = OnceLock::new();

pub(super) fn jwt_debug_enabled() -> bool {
    *JWT_DEBUG_FLAG.get_or_init(|| {
        if let Ok(val) = std::env::var("EXO_JWT_DEBUG") {
            let lowered = val.trim().to_ascii_lowercase();
            matches!(lowered.as_str(), "1" | "true" | "yes" | "on")
        } else {
            false
        }
    })
}

pub(super) fn jwt_debug_log<F>(builder: F)
where
    F: FnOnce() -> String,
{
    if jwt_debug_enabled() {
        eprintln!("[JWT Debug] {}", builder());
    }
}

fn decode_b64_segment(segment: &str) -> Option<Vec<u8>> {
    let mut normalized = segment.replace('\n', "");
    while normalized.len() % 4 != 0 {
        normalized.push('=');
    }
    URL_SAFE_NO_PAD.decode(normalized.as_bytes()).ok()
}

fn decode_jwt_header_and_payload(token: &str) -> Option<(Value, Value)> {
    let token = token.trim();
    let mut parts = token.split('.');
    let header_b64 = parts.next()?;
    let payload_b64 = parts.next()?;
    // Ensure the signature part exists even if we don't use it
    if parts.next().is_none() {
        return None;
    }

    let header_bytes = decode_b64_segment(header_b64)?;
    let payload_bytes = decode_b64_segment(payload_b64)?;

    let header = serde_json::from_slice(&header_bytes).ok()?;
    let payload = serde_json::from_slice(&payload_bytes).ok()?;

    Some((header, payload))
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
        let direct_public_key = env.get(EXO_JWT_PUBLIC_KEY_PEM);
        let public_key_envs = env.get(EXO_JWT_PUBLIC_KEY_PEM_ENVS);
        let direct_public_key_kid = env.get(EXO_JWT_PUBLIC_KEY_KID);

        // Parse allowed audiences if configured
        let allowed_audiences = env.get(EXO_JWT_AUDIENCES).map(|aud_str| {
            aud_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>()
        });

        if secret.is_some()
            && (oidc_url.is_some()
                || oidc_urls.is_some()
                || jwks_urls.is_some()
                || direct_public_key.is_some()
                || public_key_envs.is_some())
        {
            return Err(JwtConfigurationError::InvalidSetup(format!(
                "{EXO_JWT_SECRET} cannot be used with any other JWT configuration"
            )));
        }

        if oidc_url.is_some() && oidc_urls.is_some() {
            return Err(JwtConfigurationError::InvalidSetup(format!(
                "Both {EXO_OIDC_URL} and {EXO_OIDC_URLS} are set. Use only {EXO_OIDC_URLS} for multiple providers"
            )));
        }

        let mut oidc_validators = Vec::new();
        if let Some(url) = oidc_url {
            let url_for_log = url.clone();
            match Oidc::new(url).await {
                Ok(validator) => {
                    tracing::info!("Initialized OIDC provider: {}", url_for_log);
                    oidc_validators.push(validator);
                }
                Err(e) => {
                    return Err(JwtConfigurationError::Configuration {
                        message: format!(
                            "Failed to initialize OIDC provider '{}': {}",
                            url_for_log, e
                        ),
                        source: Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("{}", e),
                        )),
                    });
                }
            }
        }

        if let Some(oidc_urls_str) = oidc_urls {
            let urls: Vec<String> = oidc_urls_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if urls.is_empty() {
                return Err(JwtConfigurationError::InvalidSetup(format!(
                    "{EXO_OIDC_URLS} is set but contains no valid URLs"
                )));
            }

            for (idx, url) in urls.iter().enumerate() {
                match Oidc::new(url.clone()).await {
                    Ok(validator) => {
                        tracing::info!("Initialized OIDC provider {}: {}", idx + 1, url);
                        oidc_validators.push(validator);
                    }
                    Err(e) => {
                        return Err(JwtConfigurationError::Configuration {
                            message: format!("Failed to initialize OIDC provider '{}': {}", url, e),
                            source: Box::new(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("{}", e),
                            )),
                        });
                    }
                }
            }
        }

        let mut jwks_validators = Vec::new();
        let mut jwks_debug_snapshot = Vec::new();
        if let Some(jwks_urls_str) = jwks_urls {
            let urls: Vec<String> = jwks_urls_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if urls.is_empty() {
                return Err(JwtConfigurationError::InvalidSetup(format!(
                    "{EXO_JWKS_URLS} is set but contains no valid URLs"
                )));
            }

            for (idx, url) in urls.iter().enumerate() {
                match JwksValidator::new_with_audiences(url.clone(), allowed_audiences.clone())
                    .await
                {
                    Ok(validator) => {
                        if allowed_audiences.is_some() {
                            tracing::info!(
                                "Initialized JWKS provider {}: {} (with audience validation)",
                                idx + 1,
                                url
                            );
                        } else {
                            tracing::info!(
                                "Initialized JWKS provider {}: {} (no audience validation)",
                                idx + 1,
                                url
                            );
                        }
                        jwks_validators.push(validator);
                    }
                    Err(e) => {
                        return Err(JwtConfigurationError::Configuration {
                            message: format!("Failed to initialize JWKS provider '{}': {}", url, e),
                            source: Box::new(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("{}", e),
                            )),
                        });
                    }
                }
            }
        }

        for validator in &jwks_validators {
            jwks_debug_snapshot.push((
                validator.debug_source().to_string(),
                validator.debug_known_kids(),
                validator.debug_allowed_audiences().map(|aud| aud.to_vec()),
            ));
        }

        let mut static_key_validators = Vec::new();
        let mut static_debug_snapshot = Vec::new();
        if let Some(ref pem) = direct_public_key {
            static_key_validators.push(StaticKeyValidator::from_pem(
                EXO_JWT_PUBLIC_KEY_PEM,
                pem.clone(),
                direct_public_key_kid.clone().and_then(|kid| {
                    let trimmed = kid.trim().to_string();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    }
                }),
                allowed_audiences.clone(),
            )?);
        } else if direct_public_key_kid.is_some() {
            tracing::warn!(
                "{} is set without {}. The kid will be ignored.",
                EXO_JWT_PUBLIC_KEY_KID,
                EXO_JWT_PUBLIC_KEY_PEM
            );
        }

        if let Some(env_entries) = public_key_envs {
            let entries: Vec<String> = env_entries
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if entries.is_empty() {
                return Err(JwtConfigurationError::InvalidSetup(format!(
                    "{EXO_JWT_PUBLIC_KEY_PEM_ENVS} is set but contains no valid environment variable names"
                )));
            }

            for entry in entries {
                let (env_name_raw, kid_raw) = if let Some((name, kid)) = entry.split_once(':') {
                    (name.trim(), Some(kid.trim()))
                } else {
                    (entry.as_str(), None)
                };

                if env_name_raw.is_empty() {
                    return Err(JwtConfigurationError::InvalidSetup(format!(
                        "{EXO_JWT_PUBLIC_KEY_PEM_ENVS} contains an empty environment variable reference"
                    )));
                }

                let kid = kid_raw.and_then(|kid| {
                    let trimmed = kid.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                });

                let env_name = env_name_raw.to_string();
                let pem_value = env.get(&env_name).ok_or_else(|| {
                    JwtConfigurationError::InvalidSetup(format!(
                        "Environment variable '{}' referenced in {} is not set",
                        env_name, EXO_JWT_PUBLIC_KEY_PEM_ENVS
                    ))
                })?;

                static_key_validators.push(StaticKeyValidator::from_pem(
                    env_name,
                    pem_value,
                    kid,
                    allowed_audiences.clone(),
                )?);
            }
        }

        for validator in &static_key_validators {
            static_debug_snapshot.push((
                validator.debug_name().to_string(),
                validator.debug_kid().map(|s| s.to_string()),
                validator.debug_allowed_audiences().map(|aud| aud.to_vec()),
            ));
        }

        let style = if let Some(secret_value) = secret {
            JwtAuthenticatorStyle::Secret(secret_value)
        } else {
            let has_oidc = !oidc_validators.is_empty();
            let has_jwks = !jwks_validators.is_empty();
            let has_static = !static_key_validators.is_empty();

            if !has_oidc && !has_jwks && !has_static {
                return Ok(None);
            }

            match (has_oidc, has_jwks, has_static) {
                (true, false, false) => JwtAuthenticatorStyle::Oidc(oidc_validators),
                (false, true, false) => JwtAuthenticatorStyle::Jwks(jwks_validators),
                (false, false, true) => JwtAuthenticatorStyle::StaticKeys(static_key_validators),
                _ => JwtAuthenticatorStyle::Mixed {
                    oidc: oidc_validators,
                    jwks: jwks_validators,
                    static_keys: static_key_validators,
                },
            }
        };

        jwt_debug_log(|| {
            let style_name = match &style {
                JwtAuthenticatorStyle::Secret(_) => "secret",
                JwtAuthenticatorStyle::Oidc(_) => "oidc",
                JwtAuthenticatorStyle::Jwks(_) => "jwks",
                JwtAuthenticatorStyle::StaticKeys(_) => "static_keys",
                JwtAuthenticatorStyle::Mixed { .. } => "mixed",
            };

            format!(
                "JWT authenticator initialized with style='{style_name}', allowed_audiences={:?}",
                allowed_audiences
            )
        });

        if jwt_debug_enabled() {
            if !jwks_debug_snapshot.is_empty() {
                for (idx, (source, kids, auds)) in jwks_debug_snapshot.iter().enumerate() {
                    jwt_debug_log(|| {
                        format!(
                            "JWKS[{idx}] source='{}', kids={:?}, audience_filter={:?}",
                            source, kids, auds
                        )
                    });
                }
            }

            if !static_debug_snapshot.is_empty() {
                for (idx, (name, kid, auds)) in static_debug_snapshot.iter().enumerate() {
                    jwt_debug_log(|| {
                        format!(
                            "StaticKey[{idx}] name='{}', kid={:?}, audience_filter={:?}",
                            name, kid, auds
                        )
                    });
                }
            }
        }

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

    async fn try_oidc(validators: &[Oidc], token: &str) -> ValidationAttempt {
        let mut saw_expired = false;
        let mut had_non_kid_error = false;

        jwt_debug_log(|| {
            format!(
                "Attempting validation via {} OIDC provider(s)",
                validators.len()
            )
        });

        for (idx, validator) in validators.iter().enumerate() {
            match validator.validate(token).await {
                Ok(claims) => {
                    tracing::debug!("JWT validated successfully by OIDC provider {}", idx + 1);
                    jwt_debug_log(|| format!("OIDC provider {} accepted the token", idx + 1));
                    return ValidationAttempt {
                        outcome: ValidationOutcome::Success(claims),
                        had_non_kid_error: false,
                    };
                }
                Err(oidc_jwt_validator::ValidationError::ValidationFailed(inner)) => {
                    if matches!(
                        inner.kind(),
                        jsonwebtoken::errors::ErrorKind::ExpiredSignature
                    ) {
                        tracing::debug!("OIDC provider {} reported expired token", idx + 1);
                        saw_expired = true;
                        had_non_kid_error = true;
                        jwt_debug_log(|| {
                            format!("OIDC provider {} reported expired token", idx + 1)
                        })
                    } else {
                        tracing::debug!("OIDC provider {} validation failed: {}", idx + 1, inner);
                        had_non_kid_error = true;
                        jwt_debug_log(|| {
                            format!("OIDC provider {} validation failed: {}", idx + 1, inner)
                        })
                    }
                }
                Err(err) => {
                    had_non_kid_error = true;
                    error!(
                        "Error validating JWT with OIDC provider {}: {}",
                        idx + 1,
                        err
                    );
                    jwt_debug_log(|| {
                        format!(
                            "OIDC provider {} encountered an error during validation: {}",
                            idx + 1,
                            err
                        )
                    });
                }
            }
        }

        ValidationAttempt {
            outcome: if saw_expired {
                ValidationOutcome::Expired
            } else {
                ValidationOutcome::NoMatch
            },
            had_non_kid_error,
        }
    }

    async fn try_jwks(validators: &[JwksValidator], token: &str) -> ValidationAttempt {
        let mut saw_expired = false;
        let mut had_non_kid_error = false;

        jwt_debug_log(|| {
            format!(
                "Attempting validation via {} JWKS provider(s)",
                validators.len()
            )
        });

        for (idx, validator) in validators.iter().enumerate() {
            match validator.validate(token).await {
                Ok(claims) => {
                    tracing::debug!("JWT validated successfully by JWKS provider {}", idx + 1);
                    jwt_debug_log(|| {
                        format!(
                            "JWKS provider {} ('{}') accepted the token",
                            idx + 1,
                            validator.debug_source()
                        )
                    });
                    return ValidationAttempt {
                        outcome: ValidationOutcome::Success(claims),
                        had_non_kid_error: false,
                    };
                }
                Err(JwtValidationError::Expired) => {
                    tracing::debug!("JWKS provider {} reported expired token", idx + 1);
                    saw_expired = true;
                    had_non_kid_error = true;
                    jwt_debug_log(|| {
                        format!(
                            "JWKS provider {} ('{}') reported expired token",
                            idx + 1,
                            validator.debug_source()
                        )
                    });
                }
                Err(JwtValidationError::Invalid) => {
                    tracing::debug!("JWKS provider {} validation failed", idx + 1);
                    had_non_kid_error = true;
                    jwt_debug_log(|| {
                        format!(
                            "JWKS provider {} ('{}') failed validation (signature/audience mismatch)",
                            idx + 1,
                            validator.debug_source()
                        )
                    });
                }
                Err(JwtValidationError::KidMismatch) => {
                    tracing::debug!("JWKS provider {} skipped due to kid mismatch", idx + 1);
                    jwt_debug_log(|| {
                        format!(
                            "JWKS provider {} ('{}') skipped due to kid mismatch",
                            idx + 1,
                            validator.debug_source()
                        )
                    });
                }
            }
        }

        ValidationAttempt {
            outcome: if saw_expired {
                ValidationOutcome::Expired
            } else {
                ValidationOutcome::NoMatch
            },
            had_non_kid_error,
        }
    }

    fn try_static(validators: &[StaticKeyValidator], token: &str) -> ValidationAttempt {
        let mut saw_expired = false;
        let mut had_non_kid_error = false;

        jwt_debug_log(|| {
            format!(
                "Attempting validation via {} static public key(s)",
                validators.len()
            )
        });

        for (idx, validator) in validators.iter().enumerate() {
            match validator.validate(token) {
                Ok(claims) => {
                    tracing::debug!("JWT validated successfully by static key {}", idx + 1);
                    jwt_debug_log(|| {
                        format!(
                            "Static key {} ('{}') accepted the token",
                            idx + 1,
                            validator.debug_name()
                        )
                    });
                    return ValidationAttempt {
                        outcome: ValidationOutcome::Success(claims),
                        had_non_kid_error: false,
                    };
                }
                Err(JwtValidationError::Expired) => {
                    tracing::debug!("Static key {} reported expired token", idx + 1);
                    saw_expired = true;
                    had_non_kid_error = true;
                    jwt_debug_log(|| {
                        format!(
                            "Static key {} ('{}') reported expired token",
                            idx + 1,
                            validator.debug_name()
                        )
                    });
                }
                Err(JwtValidationError::Invalid) => {
                    tracing::debug!("Static key {} validation failed", idx + 1);
                    had_non_kid_error = true;
                    jwt_debug_log(|| {
                        format!(
                            "Static key {} ('{}') failed validation (signature/audience mismatch)",
                            idx + 1,
                            validator.debug_name()
                        )
                    });
                }
                Err(JwtValidationError::KidMismatch) => {
                    tracing::debug!("Static key {} skipped due to kid mismatch", idx + 1);
                    jwt_debug_log(|| {
                        format!(
                            "Static key {} ('{}') skipped due to kid mismatch",
                            idx + 1,
                            validator.debug_name()
                        )
                    });
                }
            }
        }

        ValidationAttempt {
            outcome: if saw_expired {
                ValidationOutcome::Expired
            } else {
                ValidationOutcome::NoMatch
            },
            had_non_kid_error,
        }
    }

    async fn validate_jwt(&self, token: &str) -> Result<Value, JwtAuthenticationError> {
        if jwt_debug_enabled() {
            if let Some((header, payload)) = decode_jwt_header_and_payload(token) {
                let kid = header
                    .get("kid")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let alg = header
                    .get("alg")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let iss = payload.get("iss").cloned();
                let aud = payload.get("aud").cloned();
                jwt_debug_log(|| {
                    format!(
                        "Token snapshot: kid={:?}, alg={:?}, iss={:?}, aud={:?}",
                        kid, alg, iss, aud
                    )
                });
            } else {
                jwt_debug_log(|| "Failed to decode token for debugging".to_string());
            }
        }

        fn map_jwt_error(error: jsonwebtoken::errors::Error) -> JwtAuthenticationError {
            match error.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    JwtAuthenticationError::Expired
                }
                _ => JwtAuthenticationError::Invalid,
            }
        }

        jwt_debug_log(|| {
            let style_desc = match &self.style {
                JwtAuthenticatorStyle::Secret(_) => "secret",
                JwtAuthenticatorStyle::Oidc(_) => "oidc",
                JwtAuthenticatorStyle::Jwks(_) => "jwks",
                JwtAuthenticatorStyle::StaticKeys(_) => "static_keys",
                JwtAuthenticatorStyle::Mixed { .. } => "mixed",
            };
            format!("Beginning JWT validation with '{style_desc}' strategy")
        });

        match &self.style {
            JwtAuthenticatorStyle::Secret(secret) => Ok(decode::<Value>(
                token,
                &DecodingKey::from_secret(secret.as_ref()),
                &Validation::default(),
            )
            .map_err(map_jwt_error)?
            .claims),

            JwtAuthenticatorStyle::Oidc(validators) => {
                let attempt = Self::try_oidc(validators, token).await;
                match attempt.outcome {
                    ValidationOutcome::Success(claims) => Ok(claims),
                    ValidationOutcome::Expired => Err(JwtAuthenticationError::Expired),
                    ValidationOutcome::NoMatch => {
                        if attempt.had_non_kid_error {
                            error!("Error validating JWT with OIDC provider(s)");
                            jwt_debug_log(|| {
                                "All OIDC providers failed validation (see above debug output)"
                                    .to_string()
                            });
                        }
                        Err(JwtAuthenticationError::Invalid)
                    }
                }
            }

            JwtAuthenticatorStyle::Jwks(validators) => {
                let attempt = Self::try_jwks(validators, token).await;
                match attempt.outcome {
                    ValidationOutcome::Success(claims) => Ok(claims),
                    ValidationOutcome::Expired => Err(JwtAuthenticationError::Expired),
                    ValidationOutcome::NoMatch => {
                        if attempt.had_non_kid_error {
                            error!("Error validating JWT with all JWKS providers");
                            jwt_debug_log(|| {
                                "All JWKS providers failed validation (see above debug output)"
                                    .to_string()
                            });
                        }
                        Err(JwtAuthenticationError::Invalid)
                    }
                }
            }

            JwtAuthenticatorStyle::StaticKeys(validators) => {
                let attempt = Self::try_static(validators, token);
                match attempt.outcome {
                    ValidationOutcome::Success(claims) => Ok(claims),
                    ValidationOutcome::Expired => Err(JwtAuthenticationError::Expired),
                    ValidationOutcome::NoMatch => {
                        if attempt.had_non_kid_error {
                            error!("Error validating JWT with configured static public keys");
                            jwt_debug_log(|| {
                                "All static public keys failed validation (see above debug output)"
                                    .to_string()
                            });
                        }
                        Err(JwtAuthenticationError::Invalid)
                    }
                }
            }

            JwtAuthenticatorStyle::Mixed {
                oidc,
                jwks,
                static_keys,
            } => {
                let mut had_non_kid_error = false;

                let oidc_attempt = Self::try_oidc(oidc, token).await;
                match oidc_attempt.outcome {
                    ValidationOutcome::Success(claims) => return Ok(claims),
                    ValidationOutcome::Expired => return Err(JwtAuthenticationError::Expired),
                    ValidationOutcome::NoMatch => {
                        had_non_kid_error |= oidc_attempt.had_non_kid_error;
                    }
                }

                let static_attempt = Self::try_static(static_keys, token);
                match static_attempt.outcome {
                    ValidationOutcome::Success(claims) => return Ok(claims),
                    ValidationOutcome::Expired => return Err(JwtAuthenticationError::Expired),
                    ValidationOutcome::NoMatch => {
                        had_non_kid_error |= static_attempt.had_non_kid_error;
                    }
                }

                let jwks_attempt = Self::try_jwks(jwks, token).await;
                match jwks_attempt.outcome {
                    ValidationOutcome::Success(claims) => return Ok(claims),
                    ValidationOutcome::Expired => return Err(JwtAuthenticationError::Expired),
                    ValidationOutcome::NoMatch => {
                        had_non_kid_error |= jwks_attempt.had_non_kid_error;
                    }
                }

                if had_non_kid_error {
                    error!("Error validating JWT with all configured providers");
                    jwt_debug_log(|| {
                        "Mixed provider validation exhausted all strategies without success"
                            .to_string()
                    });
                }
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
        jwt_debug_log(|| match &self.authenticator_source {
            AuthenticatorSource::Header(header) => {
                format!("Extracting JWT from header '{}'", header)
            }
            AuthenticatorSource::Cookie(cookie) => {
                format!("Extracting JWT from cookie '{}'", cookie)
            }
        });

        let jwt_token = self.extract_jwt_token(request_head)?;

        match jwt_token {
            Some(jwt_token) => {
                jwt_debug_log(|| format!("JWT token extracted ({} characters)", jwt_token.len()));
                self.validate_jwt(&jwt_token)
                    .await
                    .map_err(|err| {
                        jwt_debug_log(|| format!("JWT validation error: {:?}", err));
                        match &err {
                            JwtAuthenticationError::Invalid => ContextExtractionError::Unauthorized,
                            JwtAuthenticationError::Expired => {
                                ContextExtractionError::ExpiredAuthentication
                            }
                            JwtAuthenticationError::Delegate(delegate_err) => {
                                error!("Error validating JWT: {}", delegate_err);
                                ContextExtractionError::Unauthorized
                            }
                        }
                    })
                    .map(|claims| {
                        jwt_debug_log(|| {
                            if let Some(obj) = claims.as_object() {
                                let keys: Vec<&String> = obj.keys().collect();
                                format!("JWT validation succeeded; claim keys={:?}", keys)
                            } else {
                                "JWT validation succeeded; claims not an object".to_string()
                            }
                        });
                        claims
                    })
            }
            None => {
                // Either the source (header or cookie) was absent or the next token wasn't "Bearer"
                // It is not an error to have no authorization header, since that indicates an anonymous user
                // and there may be queries allowed for such users.
                jwt_debug_log(|| "No JWT token present; continuing as anonymous".to_string());
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
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde_json::json;

    use crate::{
        env_const::{
            EXO_JWT_AUDIENCES, EXO_JWT_PUBLIC_KEY_KID, EXO_JWT_PUBLIC_KEY_PEM,
            EXO_JWT_SOURCE_COOKIE, EXO_JWT_SOURCE_HEADER,
        },
        http::MemoryRequestHead,
    };

    use super::*;

    const STATIC_PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAmOyLeILOVDB/HjSzimCz
/wJ9jSIjEFmHIcsYc0MPVKy1iZItaOWd0nnEnvwhK5Gp0DWaCJ6/fe9HHOS/f9u/
xPhznI6/fmklTx9mXLyEN54lt1sIdHfI+QSNiG3UYvt3j8Le01X0ziLzdwcJ0cop
/hIGGcqmSuMqtU2+a+9hG4HbCVrKb4W3HVgAiXGV08J2FJ5Q3SbRKct5jbPZB03H
GiIVWv2yYjAEFMhClD3ALyYkGZppAkfH8EYL1+asIPlR5QEe4J6ILrxEaZe0nWxs
q6r3RHk9PZJaxfXAiMp0PPMxHTGMR1p/5x49lgBqNrcc/tK5e7l5xr9TXIdafa6R
yQIDAQAB
-----END PUBLIC KEY-----";

    const STATIC_PRIVATE_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCY7It4gs5UMH8e
NLOKYLP/An2NIiMQWYchyxhzQw9UrLWJki1o5Z3SecSe/CErkanQNZoInr9970cc
5L9/27/E+HOcjr9+aSVPH2ZcvIQ3niW3Wwh0d8j5BI2IbdRi+3ePwt7TVfTOIvN3
BwnRyin+EgYZyqZK4yq1Tb5r72EbgdsJWspvhbcdWACJcZXTwnYUnlDdJtEpy3mN
s9kHTccaIhVa/bJiMAQUyEKUPcAvJiQZmmkCR8fwRgvX5qwg+VHlAR7gnoguvERp
l7SdbGyrqvdEeT09klrF9cCIynQ88zEdMYxHWn/nHj2WAGo2txz+0rl7uXnGv1Nc
h1p9rpHJAgMBAAECggEAEHgZJSdhNSvr5MLkOxjjCamo/9QXVqFtrjQDNBaaxhG7
k09M2Kkx8ALxK/YXVGvhj+zV4+vEz7k/PVtdTFXMN1hSix/Me76zJ+xHx+D9lEfR
5AdHx9NGr5rP60t4vhg67h6chMITFUgqVD1Lz24oS5aBVbG/av1AEjqHMXScTqvo
1n1ae9bn+z8jiiCmOPuIZJStplHrn0saLBWV5fBt4is1L4ejwz7c2jpGlBU/UPNk
MTBGKjcV+opZS56Mlufs8fS3ddhgt9cFSgN/wwj/78gUtYD8VNJa+BtuYTWH3tjs
lnLzAlenE1h0mn80euyKf/hItrmJqgkF5kGQKH+YaQKBgQDNxZ7A3s0/7Lfk5Y/o
nEE7zpZzTVIaQ0HZsjeAnTwjaHGGMHKEHqLblBIetk3pc8Z9gMCX4fxcz/hf4Fsr
tgV6vSse3BkFH7FApYSLOPB6xr22CxaydNzZA8i+LYLOwJEP+ZZrLnbH851PkwXf
rlfPEeEMeknFJFTMxrqKg6hOlwKBgQC+QIkmd1pip7LLlKQTcQoQaa60CB7tQ0rS
l/VF7vQfEOXBe8KzayL0Foq7GoP9rgASX4aiiVy/uTWqlv9X4v2RAxHLsdRqv9Y8
Yi/4C3Kf0jXZ8JBk/ym59ix710HNzi6AC+bPXkm7wnt9rrEqe5FhDt18ANjN944l
Z8Foh6KOnwKBgGJrbT0u09kJbgOLUUOeyQzECO3pQ6XQGYT4WtenXQZKhFH8hilv
RdHkhq4t4CITABMzK+r5ae0yg8fH1ZOYohJMvH0sJMNwnyUehcDZYRw4RrD1qMt+
XctmpfNgbTpanIeZhzqIpMOKX+mZlqugBdvC33NKYlJqyCyRuNNbmXrNAoGAV8BE
gi2CzwYyfZvtodn9nlxgbEFiomTrWf8k7kCs8LdGgdunjkHYOWU8T9iHELb06YSO
AOICmZu/mRNUayETe5NC3gUDyMj685cGMQ52rCi1FfTTZQIcKN3W3rgGbfqvj/ft
WbBPqf6mHu44YTPldjL5DX0GgtmwAqi8mI4W+FkCgYBwM8HHehhZkbSufUlzKOFs
eURQDAKccQTat/MM7doPAZNCS0QKqUY7GtBs5oi0Y+cRfN68lN6HvnS4sTob+4OM
hypJm+8krW8P6e3NOOVJlxqgXwijZQxSM/0kuuPAka6Vclch47kZio1l1UXde/4t
GBIdO8TlPVil1Dnd9iNPpQ==
-----END PRIVATE KEY-----";

    const STATIC_KEY_KID: &str = "vreps-app-auth";

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
    async fn validates_token_with_static_public_key() {
        let env = MapEnvironment::from([
            (EXO_JWT_PUBLIC_KEY_PEM, STATIC_PUBLIC_KEY_PEM),
            (EXO_JWT_PUBLIC_KEY_KID, STATIC_KEY_KID),
            (EXO_JWT_AUDIENCES, "vreps-app"),
        ]);
        let authenticator = JwtAuthenticator::new_from_env(&env).await.unwrap().unwrap();

        let token = create_static_bearer_token();
        let request_head =
            request_head_with_headers(HashMap::from([("Authorization".to_string(), vec![token])]));

        let authentication = authenticator.extract_authentication(&request_head).await;
        let claims = authentication.unwrap();

        assert_eq!(
            claims
                .get("claims.jwt.hasura.io")
                .and_then(|value| value.get("x-hasura-user-id"))
                .and_then(Value::as_str),
            Some("15de885c-6cb0-480f-97ce-b8b8ece225d5")
        );
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

    fn create_static_bearer_token() -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(STATIC_KEY_KID.to_string());

        let current_epoch_time = {
            let start = SystemTime::now();
            let since_the_epoch = start
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards");
            since_the_epoch.as_secs() as i64
        };

        let claims = json!({
            "iss": "https://auth.vreps.tech",
            "aud": "vreps-app",
            "iat": current_epoch_time,
            "exp": current_epoch_time + 3600,
            "claims.jwt.hasura.io": {
                "x-hasura-user-id": "15de885c-6cb0-480f-97ce-b8b8ece225d5",
                "x-hasura-default-role": "app_user",
            }
        });

        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(STATIC_PRIVATE_KEY_PEM.as_bytes()).unwrap(),
        )
        .unwrap();

        format!("{}{}", TOKEN_PREFIX, token)
    }
}
