use async_graphql_parser::{
    types::{Field, OperationType},
    Positioned,
};

use crate::{
    execution::query_context::{QueryContext, QueryResponse},
    model::system::ModelSystem,
};

// TODO: Make this an implementation of FieldResolver
impl ModelSystem {
    pub fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> QueryResponse {
        match operation_type {
            OperationType::Query => {
                let operation = self.queries.get_by_key(&field.node.name.node);
                operation.unwrap().resolve(field, query_context)
            }
            OperationType::Mutation => {
                let operation = self.create_mutations.get_by_key(&field.node.name.node);
                operation.unwrap().resolve(field, query_context)
            }
            OperationType::Subscription => {
                todo!()
            }
        }
    }
}
