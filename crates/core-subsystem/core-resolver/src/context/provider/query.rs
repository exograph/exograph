// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use core_plugin_shared::trusted_documents::TrustedDocumentEnforcement;

use crate::context::context_extractor::ContextExtractor;
use crate::context::request::Request;
use crate::context::{ContextExtractionError, RequestContext};
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
impl ContextExtractor for QueryExtractor<'_> {
    fn annotation_name(&self) -> &str {
        "query"
    }

    async fn extract_context_field(
        &self,
        key: &str,
        request_context: &RequestContext,
        _request: &(dyn Request + Send + Sync),
    ) -> Result<Option<serde_json::Value>, ContextExtractionError> {
        let query = format!("query {{ {} }}", key.to_owned());

        let result = self
            .system_resolver
            .resolve_operations(
                OperationsPayload {
                    operation_name: None,
                    query: Some(query),
                    variables: None,
                    query_hash: None,
                },
                request_context,
                TrustedDocumentEnforcement::DoNotEnforce,
            )
            .await
            .map_err(|e| ContextExtractionError::Generic(e.to_string()))?;

        let matching_result = result.iter().find(|(k, _)| k == key);

        match matching_result {
            Some((_, matching_result)) => {
                let json_result = matching_result.body.to_json().map_err(|_| {
                    ContextExtractionError::Generic(
                        "Could not convert query result into JSON during @query context processing"
                            .to_string(),
                    )
                })?;

                Ok(Some(json_result))
            }
            None => Err(ContextExtractionError::Generic(format!(
                "Could not find {key} in results while processing @query context"
            ))),
        }
    }
}
