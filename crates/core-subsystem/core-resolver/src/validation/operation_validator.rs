// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_graphql_parser::{
    types::{FragmentDefinition, OperationDefinition, OperationType, VariableDefinition},
    Pos, Positioned,
};
use async_graphql_value::{ConstValue, Name};
use serde_json::{Map, Value};

use crate::{
    introspection::definition::schema::{Schema, MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME},
    validation::validation_error::ValidationError,
};

use super::{operation::ValidatedOperation, selection_set_validator::SelectionSetValidator};

/// Context for validating an operation.
pub struct OperationValidator<'a> {
    schema: &'a Schema,
    operation_name: Option<String>,
    variables: Option<Map<String, Value>>,
    fragment_definitions: HashMap<Name, Positioned<FragmentDefinition>>,
    normal_query_depth_limit: usize,
    introspection_query_depth_limit: usize,
}

impl<'a> OperationValidator<'a> {
    #[must_use]
    pub fn new(
        schema: &'a Schema,
        operation_name: Option<String>,
        variables: Option<Map<String, Value>>,
        fragment_definitions: HashMap<Name, Positioned<FragmentDefinition>>,
        normal_query_depth_limit: usize,
        introspection_query_depth_limit: usize,
    ) -> Self {
        Self {
            schema,
            operation_name,
            variables,
            fragment_definitions,
            normal_query_depth_limit,
            introspection_query_depth_limit,
        }
    }

    /// Check if the query depth is within the allowed limits
    /// Note that is_introspection is optional, since until we go one level deeper that the
    /// top-level selection set, we don't know if it's an introspection query or not.
    fn selection_depth_check(
        &self,
    ) -> impl Fn(usize, Option<bool>, Pos) -> Result<bool, ValidationError> + '_ {
        move |depth: usize,
              is_introspection: Option<bool>,
              pos: Pos|
              -> Result<bool, ValidationError> {
            if let Some(is_introspection) = is_introspection {
                let max_depth = if is_introspection {
                    self.introspection_query_depth_limit
                } else {
                    self.normal_query_depth_limit
                };
                if depth > max_depth {
                    Err(ValidationError::SelectionSetTooDeep(pos))
                } else {
                    Ok(true)
                }
            } else {
                Ok(true)
            }
        }
    }
    /// Validate operation. Operation defines a GraphQL top-level operation such
    /// as
    /// ```graphql
    ///    mutation create($name: String!) {
    ///       createName(name: $name) {
    ///          id
    ///       }
    ///    }
    /// ```
    ///
    /// Validations performed:
    /// - The operation actually exists
    /// - Each variables in [OperationDefinition.variable_definitions] is
    ///   available (see [`validate_variables`] for details)
    /// - The selected fields are valid (see [SelectionSetValidator] for details)])
    ///
    /// # Returns
    ///   A validated operation with all variables and fields resolved and normalized.
    pub(super) fn validate(
        self,
        operation: Positioned<OperationDefinition>,
    ) -> Result<ValidatedOperation, ValidationError> {
        let operation_type_name = match operation.node.ty {
            OperationType::Query => QUERY_ROOT_TYPENAME,
            OperationType::Mutation => MUTATION_ROOT_TYPENAME,
            OperationType::Subscription => todo!(),
        };

        let container_type = match self.schema.get_type_definition(operation_type_name) {
            Some(td) => td,
            None => {
                return Err(ValidationError::OperationNotFound(
                    operation_type_name.to_string(),
                    Pos::default(),
                ))
            }
        };

        let variables = self.validate_variables(operation.node.variable_definitions)?;
        let selection_set_validator = SelectionSetValidator::new(
            self.schema,
            container_type,
            &variables,
            &self.fragment_definitions,
            None,
        );

        let fields = selection_set_validator.validate(
            &operation.node.selection_set,
            0,
            &self.selection_depth_check(),
        )?;

        Ok(ValidatedOperation {
            name: self.operation_name,
            typ: operation.node.ty,
            fields,
        })
    }

    /// Validate variables.
    ///
    /// Validations performed:
    /// - All variables in [OperationDefinition.variable_definitions] are
    ///   available
    /// - (TODO) All variables are of the correct type. This is currently not
    ///   possible because we don't have enough information (such as the
    ///   `DateTime` type or the range of values for `Int`) in the schema yet.
    ///
    /// # Returns
    ///   Resolved variables (note the output type uses `ConstValue` instead of
    ///   `Value` to indicate that the value has been resolved)
    fn validate_variables(
        &'a self,
        variable_definitions: Vec<Positioned<VariableDefinition>>,
    ) -> Result<HashMap<Name, ConstValue>, ValidationError> {
        variable_definitions
            .into_iter()
            .map(|variable_definition| {
                let variable_name = variable_definition.node.name;
                let variable_value = self.var_value(&variable_name)?;
                Ok((variable_name.node, variable_value))
            })
            .collect()
    }

    fn var_value(&self, name: &Positioned<Name>) -> Result<ConstValue, ValidationError> {
        let resolved = self
            .variables
            .as_ref()
            .and_then(|variables| variables.get(name.node.as_str()))
            .ok_or_else(|| {
                ValidationError::VariableNotFound(name.node.as_str().to_string(), name.pos)
            })?;

        ConstValue::from_json(resolved.to_owned()).map_err(|e| {
            ValidationError::MalformedVariable(name.node.as_str().to_string(), name.pos, e)
        })
    }
}
