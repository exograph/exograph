use crate::{
    sql::{predicate::ConcretePredicate, select::Select, table::Table},
    transform::{pg::Postgres, transformer::OrderByTransformer, SelectionLevel},
    AbstractOrderBy, Column, Limit, Offset, PhysicalTable, Selection,
};

use super::selection_context::SelectionContext;

pub(crate) trait SelectionStrategy {
    fn id(&self) -> &'static str;

    fn suitable(&self, selection_context: &SelectionContext) -> bool;

    fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Select<'a>;
}

pub(super) fn compute_inner_select<'a>(
    table: Table<'a>,
    wildcard_table: &PhysicalTable,
    predicate: ConcretePredicate<'a>,
    order_by: &Option<AbstractOrderBy<'a>>,
    limit: &Option<Limit>,
    offset: &Option<Offset>,
    transformer: &impl OrderByTransformer,
) -> Select<'a> {
    Select {
        table,
        columns: vec![Column::Star(Some(wildcard_table.name.clone()))],
        predicate,
        order_by: order_by.as_ref().map(|ob| transformer.to_order_by(ob)),
        offset: offset.clone(),
        limit: limit.clone(),
        group_by: None,
        top_level_selection: false,
    }
}

pub(super) fn nest_subselect<'a>(
    inner_select: Select<'a>,
    selection: &Selection<'a>,
    selection_level: SelectionLevel,
    alias: &str,
    transformer: &Postgres,
) -> Select<'a> {
    let selection_aggregate = selection.selection_aggregate(transformer);

    Select {
        table: Table::SubSelect {
            select: Box::new(inner_select),
            alias: Some(alias.to_owned()),
        },
        columns: selection_aggregate,
        predicate: ConcretePredicate::True,
        order_by: None,
        offset: None,
        limit: None,
        group_by: None,
        top_level_selection: selection_level == SelectionLevel::TopLevel,
    }
}
