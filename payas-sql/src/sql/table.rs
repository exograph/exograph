use std::rc::Rc;

use super::{
    column::{Column, PhysicalColumn},
    join::Join,
    order::OrderBy,
    predicate::Predicate,
    Delete, Expression, ExpressionContext, Limit, Offset, ParameterBinding, PhysicalTable, Select,
    Update,
};
use maybe_owned::MaybeOwned;

#[derive(Debug, PartialEq)]
pub enum TableQuery<'a> {
    Physical(&'a PhysicalTable),
    Join(Join<'a>),
}

impl<'a> TableQuery<'a> {
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

    pub fn update<C, P>(
        self,
        column_values: Vec<(&'a PhysicalColumn, C)>,
        predicate: MaybeOwned<'a, Predicate<'a>>,
        returning: Vec<MaybeOwned<'a, Column<'a>>>,
    ) -> Update<'a>
    where
        C: Into<MaybeOwned<'a, Column<'a>>>,
        P: Into<MaybeOwned<'a, Predicate<'a>>>,
    {
        Update {
            table: Rc::new(self),
            column_values: column_values
                .into_iter()
                .map(|(pc, col)| (pc, Rc::new(col.into())))
                .collect(),
            predicate: Rc::new(predicate),
            returning: Rc::new(returning),
        }
    }

    pub fn delete(
        self,
        predicate: MaybeOwned<'a, Predicate<'a>>,
        returning: Vec<MaybeOwned<'a, Column<'a>>>,
    ) -> Delete<'a> {
        Delete {
            table: self,
            predicate: Rc::new(predicate),
            returning: Rc::new(returning),
        }
    }

    pub fn join(
        self,
        other_table: TableQuery<'a>,
        predicate: MaybeOwned<'a, Predicate<'a>>,
    ) -> TableQuery<'a> {
        TableQuery::Join(Join::new(self, other_table, predicate))
    }
}

impl<'a> Expression for TableQuery<'a> {
    fn binding(&self, expression_context: &mut ExpressionContext) -> ParameterBinding {
        match self {
            TableQuery::Physical(physical_table) => physical_table.binding(expression_context),
            TableQuery::Join(join) => join.binding(expression_context),
        }
    }
}
