use async_graphql_parser::types::OperationType;

use super::operation::ValidatedOperation;

/// The validated query document.
#[derive(Debug)]
pub struct ValidatedDocument {
    pub operations: Vec<ValidatedOperation>,
    pub operation_typ: OperationType,
}
