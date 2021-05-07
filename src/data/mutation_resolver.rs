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
        let operation_context = OperationContext::new(query_context);
        let selection_table = self.operation(&field.node, &operation_context, true);
        let mut expression_context = ExpressionContext::new();
        let binding = selection_table.binding(&mut expression_context);
        let string_response = query_context.system.database.execute(&binding);
        QueryResponse::Raw(string_response)
    }

    fn operation<'a>(
        &'a self,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
        top_level_selection: bool,
    ) -> Select<'a> {
        //    let table = self.physical_table(operation_context);

        //     let content_object = self.content_select(&field.selection_set, operation_context);

        //     match self.return_type.type_modifier {
        //         ModelTypeModifier::Optional | ModelTypeModifier::NonNull => {
        //             let single_column = vec![content_object];
        //             table.select(single_column, predicate, None, top_level_selection)
        //         }
        //         ModelTypeModifier::List => {
        //             let order_by = self.compute_order_by(&field.arguments, operation_context);
        //             let agg_column = operation_context.create_column(Column::JsonAgg(content_object));
        //             let vector_column = vec![agg_column];
        //             table.select(vector_column, predicate, order_by, top_level_selection)
        //         }
        //     }
        // }
        todo!()
    }
}
