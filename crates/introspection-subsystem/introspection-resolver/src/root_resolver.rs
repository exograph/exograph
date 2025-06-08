// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use common::context::RequestContext;
use common::value::Val;
use core_resolver::introspection::definition::schema::Schema;
use core_resolver::plugin::SubsystemResolutionError;
use core_resolver::validation::field::ValidatedField;

use async_graphql_parser::types::{BaseType, OperationType, Type};
use async_graphql_value::Name;
use async_trait::async_trait;
use core_resolver::introspection::definition::schema::{
    MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME, SUBSCRIPTION_ROOT_TYPENAME,
};
use serde_json::Value;

use crate::field_resolver::FieldResolver;
use crate::root_element::IntrospectionRootElement;

use super::resolver_support::Resolver;

#[async_trait]
impl FieldResolver<Value, SubsystemResolutionError> for IntrospectionRootElement<'_> {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        match self.name {
            "__type" => Ok(resolve_type(field, schema, request_context).await?),
            "__schema" => Ok(self
                .schema
                .resolve_value(&field.subfields, schema, request_context)
                .await?),
            "__typename" => {
                let typename = match self.operation_type {
                    OperationType::Query => QUERY_ROOT_TYPENAME,
                    OperationType::Mutation => MUTATION_ROOT_TYPENAME,
                    OperationType::Subscription => SUBSCRIPTION_ROOT_TYPENAME,
                };
                Ok(Value::String(typename.to_string()))
            }
            _ => {
                return Err(SubsystemResolutionError::InvalidField(
                    self.name.into(),
                    "introspection root",
                ));
            }
        }
    }
}

async fn resolve_type<'b>(
    field: &ValidatedField,
    schema: &Schema,
    request_context: &'b RequestContext<'b>,
) -> Result<Value, SubsystemResolutionError> {
    let type_name = &field
        .arguments
        .iter()
        .find(|arg| arg.0 == "name")
        .unwrap()
        .1;

    if let Val::String(name_specified) = &type_name {
        let tpe: Type = Type {
            base: BaseType::Named(Name::new(name_specified)),
            nullable: true,
        };
        tpe.resolve_value(&field.subfields, schema, request_context)
            .await
    } else {
        Ok(Value::Null)
    }
}
