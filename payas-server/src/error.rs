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

    #[error("Scalar '{0}' must not specify subfields")]
    ScalarMustNotHaveSubfields(String, Pos),

    #[error("Field '{0}' is not valid")]
    InvalidField(String, Pos),

    #[error("Required argument '{0}' not found")]
    RequiredArgumentNotFound(String, Pos),

    #[error("Argument(s) '{0:?}' invalid for this operation")]
    StrayArguments(Vec<String>, Pos),
}

impl ExecutionError {
    pub fn position(&self) -> Pos {
        match self {
            ExecutionError::VariableNotFound(_, pos) => *pos,
            ExecutionError::MalformedVariable(_, pos, _) => *pos,
            ExecutionError::FragmentDefinitionNotFound(_, pos) => *pos,
            ExecutionError::InlineFragmentNotSupported(pos) => *pos,
            ExecutionError::OperationNotFound(_, pos) => *pos,
            ExecutionError::ScalarMustNotHaveSubfields(_, pos) => *pos,
            ExecutionError::InvalidField(_, pos) => *pos,
            ExecutionError::RequiredArgumentNotFound(_, pos) => *pos,
            ExecutionError::StrayArguments(_, pos) => *pos,
        }
    }
}
