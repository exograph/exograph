use codemap_diagnostic::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Could not process input clay files")]
    Generic(Vec<Diagnostic>),
}
