use payas_model::model::{
    limit_offset::{LimitParameter, OffsetParameter},
    operation::{CreateDataParameter, UpdateDataParameter},
    order::{OrderByParameter, OrderByParameterType},
    predicate::{PredicateParameter, PredicateParameterType, PredicateParameterTypeKind},
    system::ModelSystem,
    GqlType, GqlTypeModifier,
};

use super::{
    definition::GqlFieldDefinition,
    definition::{GqlFieldTypeDefinition, GqlTypeDefinition},
};

impl GqlFieldDefinition for PredicateParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn ty<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &model.predicate_types[self.type_id]
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for PredicateParameterType {
    fn name(&self) -> &str {
        &self.name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        todo!()
    }

    fn leaf<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlTypeDefinition {
        todo!()
    }

    fn modifier(&self) -> &GqlTypeModifier {
        match &self.kind {
            PredicateParameterTypeKind::ImplicitEqual => &GqlTypeModifier::NonNull,
            PredicateParameterTypeKind::Operator(_) => &GqlTypeModifier::Optional,
            PredicateParameterTypeKind::Composite { .. } => &GqlTypeModifier::Optional,
        }
    }
}

impl GqlFieldDefinition for OrderByParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn ty<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &model.order_by_types[self.type_id]
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for OrderByParameterType {
    fn name(&self) -> &str {
        &self.name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        todo!()
    }

    fn leaf<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlTypeDefinition {
        todo!()
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &GqlTypeModifier::Optional
    }
}

impl GqlFieldDefinition for LimitParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn ty<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &model.types[self.type_id]
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldTypeDefinition for GqlType {
    fn name(&self) -> &str {
        &self.name
    }

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition> {
        todo!()
    }

    fn leaf<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlTypeDefinition {
        todo!()
    }

    fn modifier(&self) -> &GqlTypeModifier {
        &GqlTypeModifier::Optional
    }
}

impl GqlFieldDefinition for OffsetParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn ty<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &model.types[self.type_id]
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldDefinition for CreateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn ty<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &model.types[self.type_id]
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}

impl GqlFieldDefinition for UpdateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn ty<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
        &model.types[self.type_id]
    }

    fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        vec![] // Input types don't have arguments
    }
}
