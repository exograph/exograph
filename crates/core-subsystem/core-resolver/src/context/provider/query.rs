// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;

use crate::context::parsed_context::ParsedContext;
use crate::context::request::Request;
use crate::context::{ContextParsingError, RequestContext};
use crate::system_resolver::SystemResolver;
use crate::OperationsPayload;

pub struct QueryExtractor<'a> {
    system_resolver: &'a SystemResolver,
}

impl<'a> QueryExtractor<'a> {
    pub fn new(system_resolver: &'a SystemResolver) -> QueryExtractor<'a> {
        QueryExtractor { system_resolver }
    }
}

#[async_trait]
impl ParsedContext for QueryExtractor<'_> {
    fn annotation_name(&self) -> &str {
        "query"
    }

    async fn extract_context_field<'r>(
        &self,
        key: &str,
        request_context: &'r RequestContext<'r>,
        _request: &'r (dyn Request + Send + Sync),
    ) -> Result<Option<serde_json::Value>, ContextParsingError> {
        let query = format!("query {{ {} }}", key.to_owned());

        let result = self
            .system_resolver
            .resolve_operations(
                OperationsPayload {
                    operation_name: None,
                    query,
                    variables: None,
                },
                request_context,
            )
            .await
            .map_err(|e| ContextParsingError::Generic(e.to_string()))?;

        let matching_result = result.iter().find(|(k, _)| k == key);

        match matching_result {
            Some((_, matching_result)) => {
                let json_result = matching_result.body.to_json().map_err(|_| {
                    ContextParsingError::Generic(
                        "Could not convert query result into JSON during @query context processing"
                            .to_string(),
                    )
                })?;

                Ok(Some(json_result))
            }
            None => Err(ContextParsingError::Generic(format!(
                "Could not find {key} in results while processing @query context"
            ))),
        }
    }
}
