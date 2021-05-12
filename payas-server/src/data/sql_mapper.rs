use async_graphql_parser::{types::Field, Positioned};
use async_graphql_value::Value;
use payas_model::sql::SQLOperation;

use super::operation_context::OperationContext;

pub trait SQLMapper<'a, R> {
    fn map_to_sql(&'a self, argument: &'a Value, operation_context: &'a OperationContext<'a>) -> R;
}

pub trait OperationResolver<'a> {
    fn map_to_sql(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a>;
}
