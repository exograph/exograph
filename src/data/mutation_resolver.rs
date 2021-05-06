use crate::sql::{
    column::Column, predicate::Predicate, select::Select, Expression, ExpressionContext,
    PhysicalTable,
};

use crate::{execution::query_context::QueryContext, sql::order::OrderBy};

use crate::model::{operation::*, relation::*, types::*};

use super::operation_context::OperationContext;

use async_graphql_parser::{
    types::{Field, Selection, SelectionSet},
    Positioned,
};
use async_graphql_value::{Name, Value};

use crate::{execution::query_context::QueryResponse, execution::resolver::OutputName};

type Arguments = Vec<(Positioned<Name>, Positioned<Value>)>;

impl Mutation {
    pub fn resolve(
        &self,
        field: &Positioned<Field>,
        query_context: &QueryContext<'_>,
    ) -> QueryResponse {
        todo!()
    }
}
