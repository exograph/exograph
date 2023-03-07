use crate::{Limit, Offset, PhysicalTable};

use super::{
    column::Column, group_by::GroupBy, join::Join, order::OrderBy, predicate::ConcretePredicate,
    select::Select, Expression, SQLBuilder,
};
use maybe_owned::MaybeOwned;

#[derive(Debug, PartialEq)]
pub enum TableQuery<'a> {
    Physical(&'a PhysicalTable),
    Join(Join<'a>),
    SubSelect {
        select: Box<Select<'a>>,
        alias: String,
    },
}

impl<'a> TableQuery<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn select(
        self,
        columns: Vec<Column<'a>>,
        predicate: ConcretePredicate<'a>,
        order_by: Option<OrderBy<'a>>,
        offset: Option<Offset>,
        limit: Option<Limit>,
        group_by: Option<GroupBy<'a>>,
        top_level_selection: bool,
    ) -> Select<'a> {
        Select {
            underlying: self,
            columns,
            predicate,
            order_by,
            offset,
            limit,
            group_by,
            top_level_selection,
        }
    }

    pub fn join(
        self,
        other_table: TableQuery<'a>,
        predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    ) -> TableQuery<'a> {
        TableQuery::Join(Join::new(self, other_table, predicate))
    }

    pub fn base_table(&self) -> &PhysicalTable {
        match self {
            TableQuery::Physical(table) => table,
            TableQuery::Join(join) => join.left().base_table(),
            TableQuery::SubSelect { select, .. } => select.underlying.base_table(),
        }
    }
}

impl<'a> Expression for TableQuery<'a> {
    fn binding(&self, builder: &mut SQLBuilder) {
        match self {
            TableQuery::Physical(physical_table) => builder.push_quoted(&physical_table.name),
            TableQuery::Join(join) => join.binding(builder),
            TableQuery::SubSelect { select, alias } => {
                builder.push('(');
                select.binding(builder);
                builder.push(')');
                builder.push_str(" AS ");
                builder.push_quoted(alias);
            }
        }
    }
}
