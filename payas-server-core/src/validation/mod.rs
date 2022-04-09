/// Validate the query payload.
///
/// Take a user submitted query along with the operation name and variables (from the request payload)
/// and transform the query into a validated form (in the process, validate the query).
pub mod field;
pub mod operation;

pub mod document_validator;

mod operation_validator;
mod selection_set_validator;
