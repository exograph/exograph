use payas_model::model::{
    argument::ArgumentParameterType,
    mapped_arena::MappedArena,
    operation::{Mutation, Query},
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    GqlType, GqlTypeKind,
};

use crate::graphql::introspection::schema::{MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME};

use super::definition::{GqlFieldDefinition, GqlTypeDefinition};

impl GqlTypeDefinition for MappedArena<Query> {
    fn name(&self) -> &str {
        QUERY_ROOT_TYPENAME
    }

    fn fields(&self) -> Vec<&dyn GqlFieldDefinition> {
        self.values
            .iter()
            .map(|q| q.1 as &dyn GqlFieldDefinition)
            .collect()
    }
}

impl GqlTypeDefinition for MappedArena<Mutation> {
    fn name(&self) -> &str {
        MUTATION_ROOT_TYPENAME
    }

    fn fields(&self) -> Vec<&dyn GqlFieldDefinition> {
        self.values
            .iter()
            .map(|q| q.1 as &dyn GqlFieldDefinition)
            .collect()
    }
}

impl GqlTypeDefinition for GqlType {
    fn name(&self) -> &str {
        &self.name
    }

    fn fields(&self) -> Vec<&dyn GqlFieldDefinition> {
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

    fn fields(&self) -> Vec<&dyn GqlFieldDefinition> {
        vec![]
    }
}

impl GqlTypeDefinition for OrderByParameterType {
    fn name(&self) -> &str {
        &self.name
    }

    fn fields(&self) -> Vec<&dyn GqlFieldDefinition> {
        vec![]
    }
}

impl GqlTypeDefinition for ArgumentParameterType {
    fn name(&self) -> &str {
        &self.name
    }

    fn fields(&self) -> Vec<&dyn GqlFieldDefinition> {
        vec![]
    }
}
