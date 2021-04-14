use async_graphql_parser::{types::Field, Positioned};

use crate::{
    execution::query_context::{QueryContext, QueryResponse},
    model::system::ModelSystem,
};

// TODO: Make this an implementation of FieldResolver
impl ModelSystem {
    pub fn resolve(
        &self,
        field: &Positioned<Field>,
        query_context: &QueryContext<'_>,
    ) -> QueryResponse {
        let operation = self.queries.iter().find(|q| q.name == field.node.name.node);
        operation.unwrap().resolve(field, query_context)
    }
}
