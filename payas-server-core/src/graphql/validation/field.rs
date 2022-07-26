use std::collections::HashMap;

use async_graphql_value::{ConstValue, Name};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ValidatedField {
    pub alias: Option<Name>,
    /// The name of the field.
    pub name: Name,
    /// The arguments to the field, empty if no arguments are provided.
    pub arguments: HashMap<String, ConstValue>,

    /// The subfields being selected in this field, if it is an object. Empty if no fields are
    /// being selected.
    pub subfields: Vec<ValidatedField>,
}

impl ValidatedField {
    pub fn output_name(&self) -> String {
        self.alias.as_ref().unwrap_or(&self.name).to_string()
    }
}
