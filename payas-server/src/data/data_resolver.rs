use async_graphql_parser::{
    types::{Field, OperationType},
    Positioned,
};

use anyhow::{anyhow, Result};

use crate::sql::Expression;
use crate::{
    execution::query_context::{QueryContext, QueryResponse},
    sql::ExpressionContext,
};

use payas_model::model::system::ModelSystem;

use super::{operation_context::OperationContext, sql_mapper::OperationResolver};

pub trait DataResolver {
    fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> Result<QueryResponse>;
}

impl DataResolver for ModelSystem {
    fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> Result<QueryResponse> {
        let operation_context = OperationContext::new(query_context);

        let sql_operation = match operation_type {
            OperationType::Query => {
                let operation = self.queries.get_by_key(&field.node.name.node);
                operation.unwrap().map_to_sql(field, &operation_context)
            }
            OperationType::Mutation => {
                let operation = self.create_mutations.get_by_key(&field.node.name.node);
                operation.unwrap().map_to_sql(field, &operation_context)
            }
            OperationType::Subscription => {
                todo!()
            }
        };

        let mut expression_context = ExpressionContext::default();
        let binding = sql_operation.binding(&mut expression_context);
        let string_response = query_context.database.execute(&binding)?;
        Ok(QueryResponse::Raw(string_response))
    }
}
