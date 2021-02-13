use graphql_parser::{schema::InputValue, Pos};

use crate::{introspection::util, model::types::Parameter};

use super::provider::InputValueProvider;

impl<'a> InputValueProvider<'a> for Parameter {
    fn input_value(&self) -> InputValue<'a, String> {
        let field_type = util::value_type(&self.tpe.name, &self.type_modifier);

        InputValue {
            position: Pos::default(),
            description: None,
            name: self.name.clone(),
            value_type: field_type,
            default_value: None,
            directives: vec![],
        }
    }
}
