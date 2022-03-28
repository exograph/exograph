use async_graphql_value::{ConstValue, Name};

#[derive(Debug)]
pub struct ValidatedField {
    pub alias: Option<Name>,
    /// The name of the field.
    pub name: Name,
    /// The arguments to the field, empty if no arguments are provided.
    pub arguments: Vec<(String, ConstValue)>,

    /// The subfields being selected in this field, if it is an object. Empty if no fields are
    /// being selected.
    pub subfields: Vec<ValidatedField>,
}
