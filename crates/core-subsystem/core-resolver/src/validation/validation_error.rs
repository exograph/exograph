use async_graphql_parser::Pos;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("{0}")]
    QueryParsingFailed(String, Pos, Option<Pos>),

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

    #[error(
        "Argument '{argument_name}' is not of a valid type. Expected '{expected_type}', got '{actual_type}'"
    )]
    InvalidArgumentType {
        argument_name: String,
        expected_type: String,
        actual_type: String,
        pos: Pos,
    },

    #[error("No operation found")]
    NoOperationFound,

    #[error("Must provide operation name if query contains multiple operations")]
    MultipleOperationsNoOperationName,

    #[error("operationName '{0}' doesn't match any operation")]
    MultipleOperationsUnmatchedOperationName(String),
}

impl ValidationError {
    pub fn position1(&self) -> Pos {
        match self {
            ValidationError::QueryParsingFailed(_, pos, _) => *pos,
            ValidationError::VariableNotFound(_, pos) => *pos,
            ValidationError::MalformedVariable(_, pos, _) => *pos,
            ValidationError::FragmentDefinitionNotFound(_, pos) => *pos,
            ValidationError::InlineFragmentNotSupported(pos) => *pos,
            ValidationError::OperationNotFound(_, pos) => *pos,
            ValidationError::InvalidField(_, _, pos) => *pos,
            ValidationError::InvalidFieldType(_, pos) => *pos,
            ValidationError::ScalarWithField(_, pos) => *pos,
            ValidationError::RequiredArgumentNotFound(_, pos) => *pos,
            ValidationError::StrayArguments(_, _, pos) => *pos,
            ValidationError::NoOperationFound => Pos::default(),
            ValidationError::MultipleOperationsNoOperationName => Pos::default(),
            ValidationError::MultipleOperationsUnmatchedOperationName(_) => Pos::default(),
            ValidationError::InvalidArgumentType { pos, .. } => *pos,
        }
    }

    pub fn position2(&self) -> Option<Pos> {
        match self {
            ValidationError::QueryParsingFailed(_, _, pos) => *pos,
            _ => None,
        }
    }
}
