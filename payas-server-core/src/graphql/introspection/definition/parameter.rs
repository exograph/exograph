use crate::graphql::introspection::schema::SchemaInputValueDefinition;
use crate::graphql::introspection::util;

use async_graphql_value::Name;
use payas_model::model::{
    argument::ArgumentParameter,
    limit_offset::{LimitParameter, OffsetParameter},
    operation::{CreateDataParameter, UpdateDataParameter},
    order::OrderByParameter,
    predicate::PredicateParameter,
    types::GqlField,
    types::GqlTypeModifier,
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

impl Parameter for ArgumentParameter {
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
        fn input_value(&self) -> SchemaInputValueDefinition {
            let field_type = util::value_type(self.type_name(), self.type_modifier());

            SchemaInputValueDefinition {
                description: None,
                name: Name::new(self.name()),
                ty: field_type,
                default_value: None,
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

// We need to a special case for the GqlField type, so that we can properly
// created nested types such as Optional(List(List(String))). The blanket impl
// above will not work for nested types like these.
impl InputValueProvider for GqlField {
    fn input_value(&self) -> SchemaInputValueDefinition {
        let field_type = util::compute_type(&self.typ);

        SchemaInputValueDefinition {
            description: None,
            name: Name::new(&self.name),
            ty: field_type,
            default_value: None,
        }
    }
}
