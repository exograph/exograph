use async_graphql_parser::{
    types::{Field, OperationType},
    Positioned,
};

use crate::{
    execution::query_context::{QueryContext, QueryResponse},
    sql::ExpressionContext,
};
use crate::{execution::resolver::GraphQLExecutionError, sql::Expression};

use payas_model::model::system::ModelSystem;

use super::{operation_context::OperationContext, sql_mapper::OperationResolver};

pub trait DataResolver {
    fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> Result<QueryResponse, GraphQLExecutionError>;
}

impl DataResolver for ModelSystem {
    fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> Result<QueryResponse, GraphQLExecutionError> {
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
        }?;

        let mut expression_context = ExpressionContext::default();
        let binding = sql_operation.binding(&mut expression_context);
        match query_context.database.execute(&binding) {
            Ok(string_response) => Ok(QueryResponse::Raw(string_response)),
            Err(err) => Err(GraphQLExecutionError::SQLExecutionError(err)),
        }
    }
}
