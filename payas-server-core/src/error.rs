use async_graphql_parser::Pos;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutionError {
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

    #[error("No operation found")]
    NoOperationFound,

    #[error("Query and mutation may not be specified in the same document")]
    DifferentOperationTypes,
}

impl ExecutionError {
    pub fn position1(&self) -> Pos {
        match self {
            ExecutionError::QueryParsingFailed(_, pos, _) => *pos,
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
            ExecutionError::DifferentOperationTypes => Pos::default(),
        }
    }

    pub fn position2(&self) -> Option<Pos> {
        match self {
            ExecutionError::QueryParsingFailed(_, _, pos) => *pos,
            _ => None,
        }
    }
}
