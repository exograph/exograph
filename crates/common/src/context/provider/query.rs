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
// use core_plugin_shared::trusted_documents::TrustedDocumentEnforcement;

use crate::context::{context_extractor::ContextExtractor, ContextExtractionError, RequestContext};
use crate::http::{MemoryRequestHead, MemoryRequestPayload, RequestHead};
use crate::operation_payload::OperationsPayload;
use crate::router::{PlainRequestPayload, Router};

pub struct QueryExtractor<'a> {
    system_router: &'a (dyn Router<PlainRequestPayload> + Send + Sync),
}

impl<'a> QueryExtractor<'a> {
    pub fn new(
        system_router: &'a (dyn Router<PlainRequestPayload> + Send + Sync),
    ) -> QueryExtractor<'a> {
        QueryExtractor { system_router }
    }
}

#[async_trait]
impl<'a> ContextExtractor for QueryExtractor<'a> {
    fn annotation_name(&self) -> &str {
        "query"
    }

    async fn extract_context_field(
        &self,
        key: &str,
        request_context: &RequestContext,
        _request_head: &(dyn RequestHead + Send + Sync),
    ) -> Result<Option<serde_json::Value>, ContextExtractionError> {
        let query = format!("query {{ {} }}", key.to_owned());

        let operation_payload = OperationsPayload {
            operation_name: None,
            query: Some(query),
            variables: None,
            query_hash: None,
        };

        let request = MemoryRequestPayload::new(
            operation_payload.to_json().unwrap(),
            MemoryRequestHead::new(
                HashMap::new(),
                http::Method::POST,
                "/graphql".to_string(),
                serde_json::Value::Null,
                None,
            ),
        );

        let response_payload = self
            .system_router
            .route(&PlainRequestPayload::new(Box::new(request)))
            .await;

        let mut response_body_value = match response_payload {
            Some(response_payload) => {
                let response_body = response_payload.body.to_json().await.map_err(|_| {
                    ContextExtractionError::Generic(
                        "Could not convert query result into JSON during @query context processing"
                            .to_string(),
                    )
                })?;
                Ok(response_body)
            }
            None => Err(ContextExtractionError::Generic(
                "No response from system router".to_string(),
            )),
        }?;

        // The response body is a JSON object with the following structure:
        // {
        //     "data": {
        //         "<query_name>": {
        //             ... value
        //         }
        //     }
        // }

        let mut response_body_data = response_body_value["data"].take();

        if response_body_data.is_null() {
            return Err(ContextExtractionError::Generic(
                "No data in response from system router".to_string(),
            ));
        }

        let matching_result = response_body_data[key].take();

        if matching_result.is_null() {
            return Err(ContextExtractionError::Generic(format!(
                "Could not find {key} in results while processing @query context"
            )));
        }

        Ok(Some(matching_result.to_owned()))
    }
}
