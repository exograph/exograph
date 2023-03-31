use core::fmt;
use std::fmt::{Display, Formatter};

/// An issue that a user may encounter when dealing with the database schema.
///
/// Used in `model import` command.
#[derive(Debug)]
pub enum Issue {
    Warning(String),
    Hint(String),
}

impl Display for Issue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let str = match self {
            Issue::Warning(msg) => format!("warning: {msg}"),
            Issue::Hint(msg) => format!("hint: {msg}"),
        };
        write!(f, "{str}")
    }
}

/// Wraps a value with a list of issues.
#[derive(Debug)]
pub struct WithIssues<T> {
    pub value: T,
    pub issues: Vec<Issue>,
}
