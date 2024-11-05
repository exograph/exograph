use core_rest_model::path::PathTemplate;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresOperation {
    pub method: Method,
    pub path_template: PathTemplate,
    // TODO: Add parameter model
}

/// The HTTP method for the operation
///
/// We can't use http::Method, since it is not serializable.
#[derive(Serialize, Deserialize, Debug)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}
