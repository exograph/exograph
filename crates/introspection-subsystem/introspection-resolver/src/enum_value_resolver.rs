// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::types::EnumValueDefinition;
use async_trait::async_trait;
use core_resolver::introspection::definition::schema::Schema;
use core_resolver::plugin::SubsystemResolutionError;
use serde_json::Value;

use core_resolver::request_context::RequestContext;
use core_resolver::validation::field::ValidatedField;

use crate::field_resolver::FieldResolver;

#[async_trait]
impl FieldResolver<Value, SubsystemResolutionError> for EnumValueDefinition {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        _schema: &Schema,
        _request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        match field.name.as_str() {
            "name" => Ok(Value::String(self.value.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "isDeprecated" => Ok(Value::Bool(false)), // TODO
            "deprecationReason" => Ok(Value::Null),   // TODO
            "__typename" => Ok(Value::String("__EnumValue".to_string())),
            field_name => Err(SubsystemResolutionError::InvalidField(
                field_name.to_owned(),
                "EnumValueDefinition",
            )),
        }
    }
}
