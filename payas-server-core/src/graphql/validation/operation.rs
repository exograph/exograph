use async_graphql_parser::types::OperationType;

use payas_core_resolver::validation::field::ValidatedField;

// Validated operation.
#[derive(Debug)]
pub struct ValidatedOperation {
    pub name: Option<String>,
    /// The type of operation.
    pub typ: OperationType,
    /// The operation's fields.
    pub fields: Vec<ValidatedField>,
}
