// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Mutex;

use serde_json::Value;

use async_trait::async_trait;
// use core_plugin_shared::trusted_documents::TrustedDocumentEnforcement;

use crate::context::{context_extractor::ContextExtractor, ContextExtractionError, RequestContext};
use crate::http::{RequestHead, RequestPayload};
use crate::operation_payload::OperationsPayload;
use crate::router::PlainRequestPayload;

pub struct QueryExtractor;

#[async_trait]
impl<'request> ContextExtractor<'request> for QueryExtractor {
    fn annotation_name(&self) -> &str {
        "query"
    }

    async fn extract_context_field(
        &self,
        key: &str,
        request_context: &'request RequestContext<'request>,
        _request_head: &(dyn RequestHead + Send + Sync),
    ) -> Result<Option<serde_json::Value>, ContextExtractionError> {
        let query = format!("query {{ {} }}", key.to_owned());

        let operation_payload = OperationsPayload {
            operation_name: None,
            query: Some(query),
            variables: None,
            query_hash: None,
        };

        let request_head = OverriddenRequestHead {
            path: "/graphql".to_string(),
            original_head: request_context.get_head(),
        };
        let request = OverriddenRequestPayload {
            body: Mutex::new(operation_payload.to_json().unwrap()),
            head: &request_head,
        };

        let new_request_context = request_context.with_request(&request);

        let response_payload = request_context
            .get_base_context()
            .system_router
            .route(&PlainRequestPayload::internal(&new_request_context))
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

pub struct OverriddenRequestPayload<'original> {
    body: Mutex<Value>,
    head: &'original (dyn RequestHead + Send + Sync),
}

impl<'original> RequestPayload for OverriddenRequestPayload<'original> {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        self.head
    }

    fn take_body(&self) -> Value {
        self.body.lock().unwrap().take()
    }
}

struct OverriddenRequestHead<'original> {
    path: String,
    original_head: &'original (dyn RequestHead + Send + Sync),
}

impl<'original> RequestHead for OverriddenRequestHead<'original> {
    fn get_path(&self) -> String {
        self.path.clone()
    }

    fn get_headers(&self, key: &str) -> Vec<String> {
        self.original_head.get_headers(key)
    }

    fn get_ip(&self) -> Option<std::net::IpAddr> {
        self.original_head.get_ip()
    }

    fn get_query(&self) -> serde_json::Value {
        self.original_head.get_query()
    }

    fn get_method(&self) -> http::Method {
        self.original_head.get_method()
    }
}
