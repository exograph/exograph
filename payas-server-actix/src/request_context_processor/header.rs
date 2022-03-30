use super::{ContextProcessor, ContextProcessorError};

pub struct HeaderProcessor;

impl ContextProcessor for HeaderProcessor {
    fn annotation(&self) -> &str {
        "header"
    }

    fn process(
        &self,
        request: &actix_web::HttpRequest,
    ) -> Result<Vec<(String, serde_json::Value)>, super::ContextProcessorError> {
        request
            .headers()
            .iter()
            .map(|(header_name, header_value)| {
                Ok((
                    header_name.to_string().to_ascii_lowercase(),
                    header_value
                        .to_str()
                        .map_err(|_| ContextProcessorError::Malformed)?
                        .to_string()
                        .into(),
                ))
            })
            .collect::<Result<Vec<_>, ContextProcessorError>>()
    }
}
