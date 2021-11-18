use async_graphql_parser::Pos;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Variable '{0}' not found")]
    VariableNotFound(String, Pos),
}

impl ExecutionError {
    pub fn position(&self) -> Pos {
        match self {
            ExecutionError::VariableNotFound(_, pos) => *pos,
        }
    }
}
