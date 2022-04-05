use async_graphql_parser::Pos;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Variable '{0}' not found")]
    VariableNotFound(String, Pos),

    #[error("Variable '{0}' could not be deserialized: {2}")]
    MalformedVariable(String, Pos, serde_json::Error),

    #[error("Fragment definition '{0}' not found")]
    FragmentDefinitionNotFound(String, Pos),

    #[error("Inline fragments are not supported")]
    InlineFragmentNotSupported(Pos),

    #[error("No such operation '{0}'")]
    OperationNotFound(String, Pos),

    #[error("Field '{0}' is not valid for type '{1}'")]
    InvalidField(String, String, Pos),

    #[error("Field '{0}' is of a scalar type, which should not specify fields")]
    ScalarWithField(String, Pos),

    #[error("Field type '{0}' is not valid")]
    InvalidFieldType(String, Pos),

    #[error("Required argument '{0}' not found")]
    RequiredArgumentNotFound(String, Pos),

    #[error("Argument(s) '{0:?}' invalid for '{1}'")]
    StrayArguments(Vec<String>, String, Pos),

    #[error("No operation found")]
    NoOperationFound,

    #[error("Must provide operation name if query contains multiple operations")]
    MultipleOperationsNoOperationName,

    #[error("operationName '{0}' doesn't match any operation")]
    MultipleOperationsUnmatchedOperationName(String),
}

impl ExecutionError {
    pub fn position(&self) -> Pos {
        match self {
            ExecutionError::VariableNotFound(_, pos) => *pos,
            ExecutionError::MalformedVariable(_, pos, _) => *pos,
            ExecutionError::FragmentDefinitionNotFound(_, pos) => *pos,
            ExecutionError::InlineFragmentNotSupported(pos) => *pos,
            ExecutionError::OperationNotFound(_, pos) => *pos,
            ExecutionError::InvalidField(_, _, pos) => *pos,
            ExecutionError::InvalidFieldType(_, pos) => *pos,
            ExecutionError::ScalarWithField(_, pos) => *pos,
            ExecutionError::RequiredArgumentNotFound(_, pos) => *pos,
            ExecutionError::StrayArguments(_, _, pos) => *pos,
            ExecutionError::NoOperationFound => Pos::default(),
            ExecutionError::MultipleOperationsNoOperationName => Pos::default(),
            ExecutionError::MultipleOperationsUnmatchedOperationName(_) => Pos::default(),
        }
    }
}
