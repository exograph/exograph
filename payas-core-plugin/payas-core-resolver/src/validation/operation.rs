use async_graphql_parser::types::OperationType;

use crate::validation::field::ValidatedField;

// Validated operation.
#[derive(Debug)]
pub struct ValidatedOperation {
    pub name: Option<String>,
    /// The type of operation.
    pub typ: OperationType,
    /// The operation's fields (individual queries or mutations).
    pub fields: Vec<ValidatedField>,
}
