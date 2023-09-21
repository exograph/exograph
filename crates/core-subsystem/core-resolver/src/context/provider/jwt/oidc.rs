use std::time::Duration;

use oidc_jwt_validator::{cache::Strategy, ValidationError, ValidationSettings, Validator};
use serde_json::Value;

use super::authenticator::JwtConfigurationError;

pub struct Oidc {
    validator: Validator,
}

impl Oidc {
    pub(super) async fn new(url: String) -> Result<Self, JwtConfigurationError> {
        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| JwtConfigurationError::Configuration {
                message: "Unable to create client".to_owned(),
                source: e.into(),
            })?;
        let mut settings = ValidationSettings::new();
        settings.set_issuer(&[&url]);

        let validator = Validator::new(url, client, Strategy::Automatic, settings)
            .await
            .map_err(|e| JwtConfigurationError::Configuration {
                message: "Unable to create validator".to_owned(),
                source: e.into(),
            })?;

        Ok(Self { validator })
    }

    pub(super) async fn validate(&self, token: &str) -> Result<Value, ValidationError> {
        Ok(self.validator.validate(token).await?.claims)
    }
}
