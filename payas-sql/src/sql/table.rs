use super::{
    column::Column, order::OrderBy, predicate::Predicate, Expression, ExpressionContext, Limit,
    Offset, ParameterBinding, PhysicalTable, Select,
};
use maybe_owned::MaybeOwned;

#[derive(Debug, Clone, PartialEq)]
pub enum Table<'t> {
    Physical(&'t PhysicalTable),
}

impl<'a> Table<'a> {
    pub fn select<P>(
        self,
        columns: Vec<MaybeOwned<'a, Column<'a>>>,
        predicate: P,
        order_by: Option<OrderBy<'a>>,
        offset: Option<Offset>,
        limit: Option<Limit>,
        top_level_selection: bool,
    ) -> Select<'a>
    where
        P: Into<MaybeOwned<'a, Predicate<'a>>>,
    {
        Select {
            underlying: self,
            columns,
            predicate: predicate.into(),
            order_by,
            offset,
            limit,
            top_level_selection,
        }
    }
}

impl<'a> Expression for Table<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match self {
            Table::Physical(physical_table) => physical_table.binding(expression_context),
        }
    }
}
