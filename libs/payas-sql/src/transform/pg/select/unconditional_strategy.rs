use crate::{
    sql::{predicate::ConcretePredicate, select::Select, table::Table},
    transform::{
        join_util,
        transformer::{OrderByTransformer, PredicateTransformer},
    },
    Column,
};

use super::{
    selection_context::SelectionContext,
    selection_strategy::{compute_inner_select, nest_subselect, SelectionStrategy},
};

pub struct Unconditional {}

impl SelectionStrategy for Unconditional {
    fn id(&self) -> &'static str {
        "Unconditional"
    }

    fn suitable(&self, _selection_context: &SelectionContext) -> bool {
        // assert!(false, "Unconditional strategy should never be used");
        true
    }

    fn to_select<'a>(&self, selection_context: SelectionContext<'_, 'a>) -> Select<'a> {
        let SelectionContext {
            abstract_select,
            additional_predicate,
            selection_level,
            predicate_column_paths,
            order_by_column_paths,
            transformer,
            ..
        } = selection_context;
        let columns_paths = predicate_column_paths
            .into_iter()
            .chain(order_by_column_paths.into_iter())
            .collect::<Vec<_>>();

        let predicate = transformer.to_join_predicate(&abstract_select.predicate);
        let join = join_util::compute_join(abstract_select.table, &columns_paths);

        let inner_select = Select {
            table: join,
            columns: vec![Column::Physical(
                abstract_select.table.get_pk_physical_column().unwrap(),
            )],
            predicate: ConcretePredicate::In(
                Column::Physical(abstract_select.table.get_pk_physical_column().unwrap()),
                Column::SubSelect(Box::new(Select {
                    table: Table::Physical(abstract_select.table),
                    columns: vec![Column::Physical(
                        abstract_select.table.get_pk_physical_column().unwrap(),
                    )],
                    predicate,
                    order_by: abstract_select
                        .order_by
                        .as_ref()
                        .map(|ob| transformer.to_order_by(ob)),
                    offset: abstract_select.offset.clone(),
                    limit: abstract_select.limit.clone(),
                    group_by: None,
                    top_level_selection: false,
                })),
            ),
            order_by: None,
            offset: None,
            limit: None,
            group_by: None,
            top_level_selection: false,
        };

        let predicate = ConcretePredicate::In(
            Column::Physical(abstract_select.table.get_pk_physical_column().unwrap()),
            Column::SubSelect(Box::new(inner_select)),
        );
        let table = Table::Physical(abstract_select.table);

        let predicate = ConcretePredicate::and(
            predicate,
            additional_predicate.unwrap_or(ConcretePredicate::True),
        );

        // Inner select gives the data matching the predicate, order by, offset, limit
        let inner_select = compute_inner_select(
            table,
            abstract_select.table,
            predicate,
            &abstract_select.order_by,
            &abstract_select.limit,
            &abstract_select.offset,
            transformer,
        );

        nest_subselect(
            inner_select,
            &abstract_select.selection,
            selection_level,
            &abstract_select.table.name,
            transformer,
        )
    }
}
