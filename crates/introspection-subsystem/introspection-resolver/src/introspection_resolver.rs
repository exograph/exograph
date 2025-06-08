// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use async_graphql_parser::types::{FieldDefinition, OperationType, TypeDefinition};
use common::context::RequestContext;
use core_plugin_shared::interception::InterceptorIndex;
use core_resolver::{
    InterceptedOperation, QueryResponse, QueryResponseBody,
    introspection::definition::schema::Schema,
    plugin::{SubsystemGraphQLResolver, SubsystemResolutionError},
    system_resolver::GraphQLSystemResolver,
    validation::field::ValidatedField,
};

use crate::{field_resolver::FieldResolver, root_element::IntrospectionRootElement};
pub struct IntrospectionResolver {
    schema: Arc<Schema>,
}

impl IntrospectionResolver {
    pub fn new(schema: Arc<Schema>) -> Self {
        Self { schema }
    }
}

#[async_trait::async_trait]
impl SubsystemGraphQLResolver for IntrospectionResolver {
    fn id(&self) -> &'static str {
        "introspection"
    }

    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext,
        _system_resolver: &'a GraphQLSystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        let name = field.name.as_str();

        let schema = &self.schema;

        if name.starts_with("__") {
            let introspection_root = IntrospectionRootElement {
                schema,
                operation_type: &operation_type,
                name,
            };
            let body = introspection_root
                .resolve_field(field, schema, request_context)
                .await
                .map(|body| QueryResponse {
                    body: QueryResponseBody::Json(body),
                    headers: vec![],
                })?;

            Ok(Some(body))
        } else {
            Ok(None)
        }
    }

    async fn invoke_interceptor<'a>(
        &'a self,
        _interceptor_index: InterceptorIndex,
        _proceeding_interceptor: &'a InterceptedOperation<'a>,
        _request_context: &'a RequestContext<'a>,
        _system_resolver: &'a GraphQLSystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        Err(SubsystemResolutionError::NoInterceptorFound)
    }

    fn schema_queries(&self) -> Vec<FieldDefinition> {
        vec![]
    }

    fn schema_mutations(&self) -> Vec<FieldDefinition> {
        vec![]
    }

    fn schema_types(&self) -> Vec<TypeDefinition> {
        vec![]
    }
}
