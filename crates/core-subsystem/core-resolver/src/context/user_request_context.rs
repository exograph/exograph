// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use elsa::sync::FrozenMap;
use exo_sql::TransactionHolder;

use common::http::RequestHead;

use crate::{system_resolver::SystemResolver, value::Val};

use super::provider::jwt::JwtExtractor;
use super::provider::{
    cookie::CookieExtractor, environment::EnvironmentContextExtractor, header::HeaderExtractor,
    ip::IpExtractor, query::QueryExtractor,
};
use super::{
    context_extractor::BoxedContextExtractor, error::ContextExtractionError, RequestContext,
};

/// Represent a request context extracted for a particular request
pub struct UserRequestContext<'a> {
    // maps from an annotation to a parsed context
    parsed_context_map: HashMap<String, BoxedContextExtractor<'a>>,
    pub transaction_holder: Arc<Mutex<TransactionHolder>>,
    request_head: &'a (dyn RequestHead + Send + Sync),
    // cache of context values so that we compute them only once per request
    context_cache: FrozenMap<(String, String), Box<Option<Val>>>,
}

impl<'a> UserRequestContext<'a> {
    // Constructs a UserRequestContext from a vector of parsed contexts and a request.
    pub fn new(
        request_head: &'a (dyn RequestHead + Send + Sync),
        parsed_contexts: Vec<BoxedContextExtractor<'a>>,
        system_resolver: &'a SystemResolver,
    ) -> UserRequestContext<'a> {
        // a list of backend-agnostic contexts to also include
        let generic_contexts: Vec<BoxedContextExtractor> = vec![
            Box::new(EnvironmentContextExtractor {
                env: system_resolver.env.as_ref(),
            }),
            Box::new(QueryExtractor::new(system_resolver)),
            Box::new(HeaderExtractor),
            Box::new(IpExtractor),
            Box::new(CookieExtractor::new()),
            Box::new(JwtExtractor::new(system_resolver.jwt_authenticator.clone())),
        ];

        UserRequestContext {
            parsed_context_map: parsed_contexts
                .into_iter()
                .chain(generic_contexts) // include agnostic contexts
                .map(|context| (context.annotation_name().to_owned(), context))
                .collect(),
            transaction_holder: Arc::new(Mutex::new(TransactionHolder::default())),
            request_head,
            context_cache: FrozenMap::new(),
        }
    }

    pub async fn extract_context_field(
        &'a self,
        annotation: &str,
        key: &str,
        coerce_value: &impl Fn(Val) -> Result<Val, ContextExtractionError>,
        request_context: &RequestContext<'a>,
    ) -> Result<Option<&'a Val>, ContextExtractionError> {
        // Check to see if there is a cached value for this field
        // If there is, return it. Otherwise, compute it, cache it, and return it.

        // (annotation, field name), for example ("jwt", "role")
        let cache_key = (annotation.to_owned(), key.to_owned());

        // We use a double `Option` here because a value can be `None` and
        // in that case we still want to cache it.
        let cached_value: Option<&Option<Val>> = self.context_cache.get(&cache_key);

        let value: &'a Option<Val> = match cached_value {
            Some(value) => value,
            None => {
                let raw_field_value = self
                    .extract_context_field_from_source(annotation, key, request_context)
                    .await;

                let coerced_value =
                    raw_field_value.and_then(|value| value.map(coerce_value).transpose())?;

                self.context_cache
                    .insert(cache_key, Box::new(coerced_value))
            }
        };

        Ok(value.as_ref())
    }

    // Given an annotation name and its value,
    // extract a context field from the request context
    async fn extract_context_field_from_source(
        &self,
        annotation: &str,
        key: &str,
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<Val>, ContextExtractionError> {
        let parsed_context = self
            .parsed_context_map
            .get(annotation)
            .ok_or_else(|| ContextExtractionError::SourceNotFound(annotation.into()))?;

        Ok(parsed_context
            .extract_context_field(key, request_context, self.request_head)
            .await?
            .map(Val::from))
    }

    pub async fn ensure_transaction(&self) {
        self.transaction_holder
            .as_ref()
            .lock()
            .await
            .ensure_transaction();
    }
}
