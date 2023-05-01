// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_recursion::async_recursion;

use crate::{system_resolver::SystemResolver, value::Val};

use super::{
    error::ContextParsingError, parsed_context::BoxedParsedContext, request::Request,
    user_request_context::UserRequestContext,
};

use core_model::context_type::ContextField;
use serde_json::Value;

pub enum RequestContext<'a> {
    // The original request context (before any overrides)
    User(UserRequestContext<'a>),

    // The recursive nature allows stacking overrides
    Overridden {
        base_context: &'a RequestContext<'a>,
        context_override: Val,
    },
}

impl<'a> RequestContext<'a> {
    pub fn new(
        request: &'a (dyn Request + Send + Sync),
        parsed_contexts: Vec<BoxedParsedContext<'a>>,
        system_resolver: &'a SystemResolver,
    ) -> Result<RequestContext<'a>, ContextParsingError> {
        Ok(RequestContext::User(UserRequestContext::new(
            request,
            parsed_contexts,
            system_resolver,
        )?))
    }

    pub fn with_override(&'a self, context_override: Value) -> RequestContext<'a> {
        RequestContext::Overridden {
            base_context: self,
            context_override: context_override.into(),
        }
    }

    pub fn get_base_context(&self) -> &UserRequestContext {
        match &self {
            RequestContext::User(req) => req,
            RequestContext::Overridden { base_context, .. } => base_context.get_base_context(),
        }
    }

    #[async_recursion]
    pub async fn extract_context_field(
        &'a self,
        context_type_name: &str,
        field: &ContextField,
    ) -> Result<Option<&'a Val>, ContextParsingError> {
        match self {
            RequestContext::User(user_request_context) => {
                user_request_context
                    .extract_context_field(context_type_name, field, self)
                    .await
            }
            RequestContext::Overridden {
                base_context,
                context_override,
            } => {
                let overridden = context_override
                    .get(context_type_name)
                    .and_then(|value| value.get(&field.name));

                match overridden {
                    Some(value) => Ok(Some(value)),
                    None => {
                        base_context
                            .extract_context_field(context_type_name, field)
                            .await
                    }
                }
            }
        }
    }
}
