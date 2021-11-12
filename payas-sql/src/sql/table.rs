use super::{
    column::Column, join::Join, order::OrderBy, predicate::Predicate, Expression,
    ExpressionContext, Limit, Offset, ParameterBinding, PhysicalTable, Select,
};
use maybe_owned::MaybeOwned;

#[derive(Debug, PartialEq)]
pub enum Table<'a> {
    Physical(&'a PhysicalTable),
    Join(Join<'a>),
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

    pub fn join(self, other_table: Table<'a>, predicate: &'a Predicate<'a>) -> Table<'a> {
        Table::Join(Join::new(self, other_table, predicate))
    }
}

impl<'a> Expression for Table<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match self {
            Table::Physical(physical_table) => physical_table.binding(expression_context),
            Table::Join(join) => join.binding(expression_context),
        }
    }
}
