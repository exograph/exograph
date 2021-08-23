use crate::{
    execution::query_context::{QueryContext, QueryResponse},
    sql::ExpressionContext,
};
use anyhow::Result;
use async_graphql_parser::{
    types::{Field, OperationType},
    Positioned,
};

use payas_model::model::system::ModelSystem;
use payas_model::sql::OperationExpression;

use super::{
    operation_context::OperationContext,
    sql_mapper::{OperationResolver, SQLScript},
};

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

        let sql_operations = match operation_type {
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

        match sql_operations {
            SQLScript::Single(head) => {
                let mut expression_context = ExpressionContext::default();
                let binding = head.binding(&mut expression_context);
                Ok(QueryResponse::Raw(
                    query_context.database.execute(&binding)?,
                ))
            }
            SQLScript::Multi(ops) => match &ops.as_slice() {
                [init @ .., last] => {
                    for sql_operation in init {
                        let mut expression_context = ExpressionContext::default();
                        let binding = sql_operation.binding(&mut expression_context);
                        query_context.database.execute::<i32>(&binding)?; // TODO: i32 is clearly not the right type
                    }
                    let mut expression_context = ExpressionContext::default();
                    let binding = last.binding(&mut expression_context);
                    Ok(QueryResponse::Raw(
                        query_context.database.execute(&binding)?,
                    ))
                }
                _ => panic!("SQLScript::Multi variant didn't have multiple operations"),
            },
        }
    }
}
