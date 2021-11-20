use codemap_diagnostic::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Could not process input clay files")]
    Diagosis(Vec<Diagnostic>),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
