use async_graphql_parser::types::{BaseType, Type};
use payas_model::model::{
    argument::{ArgumentParameter, ArgumentParameterType, ArgumentParameterTypeWithModifier},
    limit_offset::{LimitParameter, LimitParameterType, OffsetParameter, OffsetParameterType},
    operation::{CreateDataParameter, CreateDataParameterTypeWithModifier, UpdateDataParameter},
    order::{OrderByParameter, OrderByParameterType, OrderByParameterTypeWithModifier},
    predicate::{PredicateParameter, PredicateParameterType, PredicateParameterTypeWithModifier},
    system::ModelSystem,
    GqlType, GqlTypeModifier,
};

use super::{definition::GqlFieldDefinition, definition::GqlFieldTypeDefinition};

impl GqlFieldDefinition for PredicateParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.typ
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for PredicateParameterTypeWithModifier {
    fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
        &model.predicate_types[self.type_id].name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        if self.type_modifier == GqlTypeModifier::NonNull {
            None
        } else {
            Some(&model.predicate_types[self.type_id])
        }
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl GqlFieldTypeDefinition for PredicateParameterType {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.name
    }

    fn inner<'a>(&'a self, _model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        None
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &GqlTypeModifier::NonNull
    }
}

impl GqlFieldDefinition for OrderByParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.typ
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for OrderByParameterTypeWithModifier {
    fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
        &model.order_by_types[self.type_id].name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        if self.type_modifier == GqlTypeModifier::NonNull {
            None
        } else {
            Some(&model.order_by_types[self.type_id])
        }
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl GqlFieldTypeDefinition for OrderByParameterType {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.name
    }

    fn inner<'a>(&'a self, _model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        None
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &GqlTypeModifier::NonNull
    }
}

impl GqlFieldDefinition for LimitParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.typ
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for LimitParameterType {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.type_name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        Some(&model.types[self.type_id])
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl GqlFieldTypeDefinition for GqlType {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.name
    }

    fn inner<'a>(&'a self, _model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        None
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &GqlTypeModifier::NonNull
    }
}

impl GqlFieldDefinition for OffsetParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.typ
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for OffsetParameterType {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.type_name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        Some(&model.types[self.type_id])
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl GqlFieldDefinition for CreateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.typ
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for CreateDataParameterTypeWithModifier {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.type_name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        if self.array_input {
            Some(&model.mutation_types[self.type_id])
        } else {
            None
        }
    }

    fn modifier(&self) -> &GqlTypeModifier {
        if self.array_input {
            &GqlTypeModifier::List
        } else {
            &GqlTypeModifier::NonNull
        }
    }
}

impl GqlFieldDefinition for UpdateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &model.mutation_types[self.type_id]
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldDefinition for ArgumentParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &self.typ
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for ArgumentParameterTypeWithModifier {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.type_name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        if self.type_modifier == GqlTypeModifier::NonNull {
            None
        } else {
            self.type_id
                .as_ref()
                .map(|t| &model.argument_types[*t] as &dyn GqlFieldTypeDefinition)
        }
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &self.type_modifier
    }
}

impl GqlFieldTypeDefinition for ArgumentParameterType {
    fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
        &self.name
    }

    fn inner<'a>(&'a self, _model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        None
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &GqlTypeModifier::NonNull
    }
}

impl GqlFieldTypeDefinition for Type {
    fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
        match &self.base {
            BaseType::Named(name) => name.as_str(),
            BaseType::List(underlying) => underlying.name(model),
        }
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        if self.nullable {
            Some(&self.base)
        } else {
            self.base.inner(model)
        }
    }

    fn modifier(&self) -> &GqlTypeModifier {
        if self.nullable {
            &GqlTypeModifier::Optional
        } else {
            self.base.modifier()
        }
    }
}

impl GqlFieldTypeDefinition for BaseType {
    fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
        match self {
            BaseType::Named(name) => name.as_str(),
            BaseType::List(underlying) => underlying.name(model),
        }
    }

    fn inner<'a>(&'a self, _model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        match self {
            BaseType::Named(_) => None,
            BaseType::List(underlying) => Some(underlying.as_ref()),
        }
    }

    fn modifier(&self) -> &GqlTypeModifier {
        match self {
            BaseType::Named(_) => &GqlTypeModifier::NonNull,
            BaseType::List(_) => &GqlTypeModifier::List,
        }
    }
}
