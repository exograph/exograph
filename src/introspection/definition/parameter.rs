use graphql_parser::{schema::InputValue, Pos};

use crate::{introspection::util, model::{order::*, predicate::PredicateParameter, types::ModelTypeModifier}};

use super::provider::InputValueProvider;

pub trait Parameter {
    fn name(&self) -> &String;
    fn type_name(&self) -> &String;
    fn type_modifier(&self) -> &ModelTypeModifier;
}

impl Parameter for OrderByParameter {
    fn name(&self) -> &String {
        &self.name
    }

    fn type_name(&self) -> &String {
        &self.type_name
    }

    fn type_modifier(&self) -> &ModelTypeModifier {
        &self.type_modifier
    }
}

impl Parameter for PredicateParameter {
    fn name(&self) -> &String {
        &self.name
    }

    fn type_name(&self) -> &String {
        &self.type_name
    }

    fn type_modifier(&self) -> &ModelTypeModifier {
        &self.type_modifier
    }
}

impl<'a, T: Parameter> InputValueProvider<'a> for T {
    fn input_value(&self) -> InputValue<'a, String> {
        let field_type = util::value_type(&self.type_name(), &self.type_modifier());

        InputValue {
            position: Pos::default(),
            description: None,
            name: self.name().clone(),
            value_type: field_type,
            default_value: None,
            directives: vec![],
        }
    }
}

// TODO: Derive this from the one above
impl<'a> InputValueProvider<'a> for &dyn Parameter {
    fn input_value(&self) -> InputValue<'a, String> {
        let field_type = util::value_type(&self.type_name(), &self.type_modifier());

        InputValue {
            position: Pos::default(),
            description: None,
            name: self.name().clone(),
            value_type: field_type,
            default_value: None,
            directives: vec![],
        }
    }
}
