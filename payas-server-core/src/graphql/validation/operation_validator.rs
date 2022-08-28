use std::collections::HashMap;

use async_graphql_parser::{
    types::{FragmentDefinition, OperationDefinition, OperationType, VariableDefinition},
    Positioned,
};
use async_graphql_value::{ConstValue, Name};
use payas_model::model::system::ModelSystem;
use serde_json::{Map, Value};

use crate::graphql::validation::{
    definition::GqlTypeDefinition, validation_error::ValidationError,
};

use super::{operation::ValidatedOperation, selection_set_validator::SelectionSetValidator};

/// Context for validating an operation.
pub struct OperationValidator<'a> {
    model: &'a ModelSystem,
    operation_name: Option<String>,
    variables: Option<Map<String, Value>>,
    fragment_definitions: HashMap<Name, Positioned<FragmentDefinition>>,
}

impl<'a> OperationValidator<'a> {
    #[must_use]
    pub fn new(
        model: &'a ModelSystem,
        operation_name: Option<String>,
        variables: Option<Map<String, Value>>,
        fragment_definitions: HashMap<Name, Positioned<FragmentDefinition>>,
    ) -> Self {
        Self {
            model,
            operation_name,
            variables,
            fragment_definitions,
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
        let container_type_definition: &dyn GqlTypeDefinition = match operation.node.ty {
            OperationType::Query => &self.model.queries,
            OperationType::Mutation => &self.model.mutations,
            OperationType::Subscription => todo!(),
        };

        let variables = self.validate_variables(operation.node.variable_definitions)?;
        let selection_set_validator = SelectionSetValidator::new(
            self.model,
            container_type_definition,
            &variables,
            &self.fragment_definitions,
        );

        let fields = selection_set_validator.validate(&operation.node.selection_set)?;

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
