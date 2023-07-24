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
    error::ContextParsingError, parsed_context::ContextExtractor, request::Request, RequestContext,
};

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

    pub fn extract_cookies(
        request: &dyn Request,
    ) -> Result<HashMap<String, Value>, ContextParsingError> {
        let cookie_headers = request.get_headers("cookie");

        let cookie_strings = cookie_headers
            .into_iter()
            .map(|header| header.split(';').collect());

        let cookies = cookie_strings
            .map(|cookie_string: String| {
                Cookie::parse(cookie_string)
                    .map(|cookie| (cookie.name().to_owned(), cookie.value().to_owned().into()))
                    .map_err(|_| ContextParsingError::Malformed)
            })
            .collect::<Result<Vec<(String, Value)>, ContextParsingError>>()?;

        Ok(cookies.into_iter().collect())
    }
}

#[async_trait]
impl ContextExtractor for CookieExtractor {
    fn annotation_name(&self) -> &str {
        "cookie"
    }

    async fn extract_context_field<'r>(
        &self,
        key: &str,
        _request_context: &RequestContext,
        request: &(dyn Request + Send + Sync),
    ) -> Result<Option<Value>, ContextParsingError> {
        Ok(self
            .extracted_cookies
            .get_or_try_init(|| futures::future::ready(Self::extract_cookies(request)))
            .await?
            .get(key)
            .cloned())
    }
}
