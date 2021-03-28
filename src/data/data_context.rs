use async_graphql_parser::{Positioned, types::Field};

use crate::{
    execution::query_context::{QueryContext, QueryResponse},
    model::system::ModelSystem,
    sql::database::Database,
};
#[derive(Debug)]
pub struct DataContext<'a> {
    pub system: ModelSystem,
    pub database: Database<'a>,
}

impl<'a> DataContext<'a> {
    pub fn resolve(&self, field: &Positioned<Field>, query_context: &QueryContext<'_>) -> QueryResponse {
        let operation = self
            .system
            .queries
            .iter()
            .find(|q| q.name == field.node.name.node);
        operation.unwrap().resolve(field, query_context)
    }
}
