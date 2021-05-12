use crate::introspection::util::*;
use async_graphql_parser::types::InputValueDefinition;

use crate::introspection::util;

use payas_model::model::{
    operation::MutationDataParameter, order::*, predicate::PredicateParameter, types::ModelField,
    types::ModelTypeModifier,
};

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

impl Parameter for MutationDataParameter {
    fn name(&self) -> &String {
        &self.name
    }

    fn type_name(&self) -> &String {
        &self.type_name
    }

    fn type_modifier(&self) -> &ModelTypeModifier {
        &ModelTypeModifier::NonNull
    }
}

impl Parameter for ModelField {
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

impl<T: Parameter> InputValueProvider for T {
    fn input_value(&self) -> InputValueDefinition {
        let field_type =
            util::default_positioned(util::value_type(&self.type_name(), &self.type_modifier()));

        InputValueDefinition {
            description: None,
            name: default_positioned_name(self.name()),
            ty: field_type,
            default_value: None,
            directives: vec![],
        }
    }
}

// TODO: Derive this from the one above
impl InputValueProvider for &dyn Parameter {
    fn input_value(&self) -> InputValueDefinition {
        let field_type =
            util::default_positioned(util::value_type(&self.type_name(), &self.type_modifier()));

        InputValueDefinition {
            description: None,
            name: default_positioned_name(self.name()),
            ty: field_type,
            default_value: None,
            directives: vec![],
        }
    }
}
