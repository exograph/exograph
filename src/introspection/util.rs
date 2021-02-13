use graphql_parser::{query::Field, schema::Type};

use crate::model::types::{ModelTypeModifier, ModelTypeModifier::*};

pub fn value_type<'a>(name: &str, type_modifier: &ModelTypeModifier) -> Type<'a, String> {
    let base_field_type = Type::NamedType(name.to_owned());
    match type_modifier {
        Optional => base_field_type,
        NonNull => Type::NonNullType(Box::new(base_field_type)),
        List => Type::ListType(Box::new(base_field_type)),
    }
}
pub trait OutputName<'a> {
    fn output_name(&self) -> String;
}

impl<'a> OutputName<'a> for Field<'a, String> {
    fn output_name(&self) -> String {
        self.alias.clone().unwrap_or(self.name.clone())
    }
}
