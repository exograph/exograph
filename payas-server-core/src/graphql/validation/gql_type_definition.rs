use async_graphql_parser::types::{TypeDefinition, TypeKind};
use payas_model::model::{
    argument::ArgumentParameterType,
    mapped_arena::SerializableSlab,
    operation::{Mutation, Query},
    order::{OrderByParameterType, OrderByParameterTypeKind},
    predicate::{PredicateParameterType, PredicateParameterTypeKind},
    system::ModelSystem,
    GqlType, GqlTypeKind,
};

use crate::graphql::introspection::definition::schema::{
    MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME,
};

use super::definition::{GqlFieldDefinition, GqlTypeDefinition};

impl GqlTypeDefinition for (&SerializableSlab<Query>, &SerializableSlab<Query>) {
    fn name(&self) -> &str {
        QUERY_ROOT_TYPENAME
    }

    fn fields(&self, _model: &ModelSystem) -> Vec<&dyn GqlFieldDefinition> {
        self.0
            .iter()
            .chain(self.1.iter())
            .map(|q| q.1 as &dyn GqlFieldDefinition)
            .collect()
    }
}

impl GqlTypeDefinition for (&SerializableSlab<Mutation>, &SerializableSlab<Mutation>) {
    fn name(&self) -> &str {
        MUTATION_ROOT_TYPENAME
    }

    fn fields(&self, _model: &ModelSystem) -> Vec<&dyn GqlFieldDefinition> {
        self.0
            .iter()
            .chain(self.1.iter())
            .map(|q| q.1 as &dyn GqlFieldDefinition)
            .collect()
    }
}

impl GqlTypeDefinition for GqlType {
    fn name(&self) -> &str {
        &self.name
    }

    fn fields(&self, _model: &ModelSystem) -> Vec<&dyn GqlFieldDefinition> {
        match &self.kind {
            GqlTypeKind::Primitive => vec![],
            GqlTypeKind::Composite(composite_type) => composite_type
                .fields
                .iter()
                .map(|f| f as &dyn GqlFieldDefinition)
                .collect(),
        }
    }
}

impl GqlTypeDefinition for PredicateParameterType {
    fn name(&self) -> &str {
        &self.name
    }

    fn fields(&self, _model: &ModelSystem) -> Vec<&dyn GqlFieldDefinition> {
        match &self.kind {
            PredicateParameterTypeKind::Operator(parameters) => parameters
                .iter()
                .map(|parameter| parameter as &dyn GqlFieldDefinition)
                .collect(),
            PredicateParameterTypeKind::Composite {
                field_params,
                logical_op_params,
            } => field_params
                .iter()
                .chain(logical_op_params.iter())
                .map(|parameter| parameter as &dyn GqlFieldDefinition)
                .collect(),
            PredicateParameterTypeKind::ImplicitEqual => vec![],
        }
    }
}

impl GqlTypeDefinition for OrderByParameterType {
    fn name(&self) -> &str {
        &self.name
    }

    fn fields(&self, _model: &ModelSystem) -> Vec<&dyn GqlFieldDefinition> {
        match &self.kind {
            OrderByParameterTypeKind::Primitive => vec![],
            OrderByParameterTypeKind::Composite { parameters } => parameters
                .iter()
                .map(|parameter| parameter as &dyn GqlFieldDefinition)
                .collect(),
        }
    }
}

impl GqlTypeDefinition for ArgumentParameterType {
    fn name(&self) -> &str {
        &self.name
    }

    fn fields<'a>(&'a self, model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        let underlying_type = if self.is_primitive {
            &model.primitive_types[self.actual_type_id.unwrap()]
        } else {
            &model.service_types[self.actual_type_id.unwrap()]
        };

        match &underlying_type.kind {
            GqlTypeKind::Primitive => vec![],
            GqlTypeKind::Composite(composite_type) => composite_type
                .fields
                .iter()
                .map(|f| f as &dyn GqlFieldDefinition)
                .collect(),
        }
    }
}

impl GqlTypeDefinition for TypeDefinition {
    fn name(&self) -> &str {
        self.name.node.as_str()
    }

    fn fields<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
        match &self.kind {
            TypeKind::Scalar | TypeKind::Interface(_) | TypeKind::Union(_) => vec![],
            TypeKind::Object(obj) => obj
                .fields
                .iter()
                .map(|f| &f.node as &dyn GqlFieldDefinition)
                .collect(),

            TypeKind::Enum(_) => vec![],
            TypeKind::InputObject(obj) => obj
                .fields
                .iter()
                .map(|f| &f.node as &dyn GqlFieldDefinition)
                .collect(),
        }
    }
}
