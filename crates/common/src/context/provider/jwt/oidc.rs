use oidc_jwt_validator::{cache::Strategy, ValidationError, ValidationSettings, Validator};
use serde_json::Value;

use super::authenticator::JwtConfigurationError;

pub struct Oidc {
    validator: Validator,
}

impl Oidc {
    pub(super) async fn new(url: String) -> Result<Self, JwtConfigurationError> {
        let client = reqwest::ClientBuilder::new().build().map_err(|e| {
            JwtConfigurationError::Configuration {
                message: "Unable to create HTTP client".to_owned(),
                source: e.into(),
            }
        })?;
        let mut settings = ValidationSettings::new();

        // The issuer can be either the base URL (for example, Clerk) or the base URL with a trailing slash (for example, Auth0)
        // so we add both to the list of issuers to check.
        let url = url.trim_end_matches('/').to_owned();
        settings.set_issuer(&[&url, &format!("{url}/")]);

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
