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
}

impl ExecutionError {
    pub fn position(&self) -> Pos {
        match self {
            ExecutionError::VariableNotFound(_, pos) => *pos,
            ExecutionError::MalformedVariable(_, pos, _) => *pos,
            ExecutionError::FragmentDefinitionNotFound(_, pos) => *pos,
        }
    }
}
