use async_graphql_parser::types::OperationType;

use super::field::ValidatedField;

// Validated operation
// Validations performed:
// - Validate that each variables in [OperationDefinition.variable_definitions] is available
#[derive(Debug)]
pub struct ValidatedOperationDefinition {
    pub name: Option<String>,
    /// The type of operation.
    pub typ: OperationType,
    /// The operation's fields.
    pub fields: Vec<ValidatedField>,
}
