// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::types::FieldDefinition;
use async_trait::async_trait;
use core_resolver::introspection::definition::schema::Schema;
use core_resolver::plugin::SubsystemResolutionError;
use serde_json::Value;

use common::context::RequestContext;
use core_resolver::validation::field::ValidatedField;

use crate::field_resolver::FieldResolver;

use super::resolver_support::Resolver;

#[async_trait]
impl FieldResolver<Value, SubsystemResolutionError> for FieldDefinition {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        match field.name.as_str() {
            "name" => Ok(Value::String(self.name.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "type" => {
                self.ty
                    .resolve_value(&field.subfields, schema, request_context)
                    .await
            }
            "args" => {
                self.arguments
                    .resolve_value(&field.subfields, schema, request_context)
                    .await
            }
            "isDeprecated" => Ok(Value::Bool(false)), // TODO
            "deprecationReason" => Ok(Value::Null),   // TODO
            "__typename" => Ok(Value::String("__Field".to_string())),
            field_name => Err(SubsystemResolutionError::InvalidField(
                field_name.to_owned(),
                "Field",
            )),
        }
    }
}
