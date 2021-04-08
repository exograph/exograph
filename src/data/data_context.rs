use async_graphql_parser::{types::Field, Positioned};

use crate::{
    execution::query_context::{QueryContext, QueryResponse},
    model::{system::ModelSystem, types::ModelTypeKind},
    sql::{database::Database, table::PhysicalTable},
};
#[derive(Debug)]
pub struct DataContext<'a> {
    pub system: ModelSystem,
    pub database: Database<'a>,
}

impl<'a> DataContext<'a> {
    pub fn resolve(
        &self,
        field: &Positioned<Field>,
        query_context: &QueryContext<'_>,
    ) -> QueryResponse {
        let operation = self
            .system
            .queries
            .iter()
            .find(|q| q.name == field.node.name.node);
        operation.unwrap().resolve(field, query_context)
    }

    pub fn physical_table(&'a self, type_name: &str) -> Option<&'a PhysicalTable<'a>> {
        self.system
            .find_type(type_name)
            .map(|t| match &t.kind {
                ModelTypeKind::Composite {
                    model_fields: _,
                    table_name,
                } => Some(table_name),
                _ => None,
            })
            .flatten()
            .map(|table_name| self.database.get_table(table_name))
            .flatten()
    }
}
