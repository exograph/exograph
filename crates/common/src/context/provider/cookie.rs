// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_trait::async_trait;
use cookie::Cookie;
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::context::{
    RequestContext, context_extractor::ContextExtractor, error::ContextExtractionError,
};
use crate::http::RequestHead;

pub struct CookieExtractor {
    // Use OnceCell to process cookies only once per request (and not per cookie annotation)
    extracted_cookies: OnceCell<HashMap<String, Value>>,
}

impl CookieExtractor {
    pub fn new() -> Self {
        Self {
            extracted_cookies: OnceCell::new(),
        }
    }

    pub fn extract_cookies<T: for<'a> From<&'a str>>(
        request_head: &dyn RequestHead,
    ) -> Result<HashMap<String, T>, ContextExtractionError> {
        let mut result = vec![];
        for header in request_head.get_headers("cookie") {
            for cookie_string in header.split(';') {
                if !cookie_string.is_empty() {
                    let cookie = Cookie::parse(cookie_string)
                        .map_err(|_| ContextExtractionError::Malformed)?;

                    let (name, value) = (cookie.name().to_owned(), T::from(cookie.value()));
                    result.push((name, value));
                }
            }
        }

        Ok(result.into_iter().collect())
    }
}

#[async_trait]
impl ContextExtractor for CookieExtractor {
    fn annotation_name(&self) -> &str {
        "cookie"
    }

    async fn extract_context_field(
        &self,
        key: &str,
        request_context: &RequestContext,
    ) -> Result<Option<Value>, ContextExtractionError> {
        use crate::http::RequestPayload;

        Ok(self
            .extracted_cookies
            .get_or_try_init(|| async { Self::extract_cookies(request_context.get_head()) })
            .await?
            .get(key)
            .cloned())
    }
}
#[cfg(test)]
mod tests {
    use crate::http::MemoryRequestHead;

    use super::*;

    #[test]
    fn test_extract_cookies_empty() {
        let request_head = form_cookie_header_request_head(vec![]);
        let cookies = CookieExtractor::extract_cookies::<String>(&request_head).unwrap();
        assert_eq!(cookies.len(), 0);
    }

    #[test]
    fn test_extract_cookies_single() {
        let request_head = form_cookie_header_request_head(vec![("name1", "value1")]);
        let cookies = CookieExtractor::extract_cookies(&request_head).unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies.get("name1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_extract_cookies_multiple() {
        let request_head =
            form_cookie_header_request_head(vec![("name1", "value1"), ("name2", "value2")]);

        let cookies = CookieExtractor::extract_cookies(&request_head).unwrap();

        assert_eq!(cookies.len(), 2);
        assert_eq!(
            cookies.get("name1"),
            Some(&Value::String("value1".to_string()))
        );
        assert_eq!(
            cookies.get("name2"),
            Some(&Value::String("value2".to_string()))
        );
    }

    fn form_cookie_header_request_head(cookies: Vec<(&str, &str)>) -> MemoryRequestHead {
        MemoryRequestHead::new(
            HashMap::new(),
            HashMap::from_iter(
                cookies
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string())),
            ),
            http::Method::GET,
            "/".to_string(),
            serde_json::Value::Null,
            None,
        )
    }
}
