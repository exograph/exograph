use crate::introspection::util::*;
use async_graphql_parser::types::InputValueDefinition;

use crate::introspection::util;

use payas_model::model::{
    limit_offset::{LimitParameter, OffsetParameter},
    operation::{CreateDataParameter, UpdateDataParameter},
    order::*,
    predicate::PredicateParameter,
    service::ServiceMethodArgument,
    types::GqlField,
    types::GqlTypeModifier,
    GqlFieldType,
};

use super::provider::InputValueProvider;

pub trait Parameter {
    fn name(&self) -> &str;
    fn type_name(&self) -> &str;
    fn type_modifier(&self) -> &GqlTypeModifier;
}

impl Parameter for OrderByParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl Parameter for LimitParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl Parameter for OffsetParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl Parameter for PredicateParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl Parameter for CreateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> &GqlTypeModifier {
        if self.array_input {
            &GqlTypeModifier::List
        } else {
            &GqlTypeModifier::NonNull
        }
    }
}

impl Parameter for UpdateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> &GqlTypeModifier {
        &GqlTypeModifier::NonNull
    }
}

impl Parameter for GqlField {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        self.typ.type_name()
    }

    fn type_modifier(&self) -> &GqlTypeModifier {
        match self.typ {
            GqlFieldType::Optional(_) => &GqlTypeModifier::Optional,
            GqlFieldType::Reference { .. } => &GqlTypeModifier::NonNull,
            GqlFieldType::List(_) => &GqlTypeModifier::List,
        }
    }
}

impl Parameter for ServiceMethodArgument {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

macro_rules! parameter_input_value_provider {
    () => {
        fn input_value(&self) -> InputValueDefinition {
            let field_type =
                util::default_positioned(util::value_type(self.type_name(), self.type_modifier()));

            InputValueDefinition {
                description: None,
                name: default_positioned_name(self.name()),
                ty: field_type,
                default_value: None,
                directives: vec![],
            }
        }
    };
}

impl<T: Parameter> InputValueProvider for T {
    parameter_input_value_provider!();
}

// TODO: Derive this from the one above
impl InputValueProvider for &dyn Parameter {
    parameter_input_value_provider!();
}
