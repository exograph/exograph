use crate::{
    data::operation_mapper::OperationResolverResult,
    execution::query_context::{QueryContext, QueryResponse},
};
use anyhow::{bail, Result};
use async_graphql_parser::{
    types::{Field, OperationType},
    Positioned,
};

use payas_model::model::system::ModelSystem;
use postgres::{types::FromSqlOwned, Row};

use super::{operation_context::OperationContext, operation_mapper::OperationResolver};

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

        let resolver_result = match operation_type {
            OperationType::Query => {
                let operation = self.queries.get_by_key(&field.node.name.node);
                operation
                    .unwrap()
                    .resolve_operation(field, &operation_context)
            }
            OperationType::Mutation => {
                let operation = self.create_mutations.get_by_key(&field.node.name.node);
                operation
                    .unwrap()
                    .resolve_operation(field, &operation_context)
            }
            OperationType::Subscription => {
                todo!()
            }
        }?;

        match resolver_result {
            OperationResolverResult::SQLOperation(transaction_script) => {
                let mut client = query_context.executor.database.get_client()?;
                let mut result = transaction_script.execute(&mut client, extractor)?;

                if result.len() == 1 {
                    Ok(QueryResponse::Raw(Some(result.swap_remove(0))))
                } else if result.is_empty() {
                    Ok(QueryResponse::Raw(None))
                } else {
                    bail!(format!(
                        "Result has {} entries; expected only zero or one",
                        result.len()
                    ))
                }
            }

            OperationResolverResult::DenoOperation(_) => {
                todo!()
            }
        }
    }
}

pub fn extractor<T: FromSqlOwned>(row: Row) -> Result<T> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => bail!("Got row without any columns {}", err),
    }
}
