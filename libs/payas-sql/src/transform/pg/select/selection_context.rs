use crate::{
    sql::predicate::ConcretePredicate,
    transform::{pg::Postgres, SelectionLevel},
    AbstractSelect, ColumnPath, ColumnPathLink, Selection,
};

pub(crate) struct SelectionContext<'c, 'a> {
    pub abstract_select: &'c AbstractSelect<'a>,
    pub has_a_one_to_many_clause: bool,
    pub predicate_column_paths: Vec<Vec<ColumnPathLink<'a>>>,
    pub order_by_column_paths: Vec<Vec<ColumnPathLink<'a>>>,
    pub additional_predicate: Option<ConcretePredicate<'a>>,
    pub is_return_value_agg: bool,
    pub selection_level: SelectionLevel,
    pub transformer: &'c Postgres,
}

impl<'c, 'a> SelectionContext<'c, 'a> {
    pub fn new(
        abstract_select: &'c AbstractSelect<'a>,
        additional_predicate: Option<ConcretePredicate<'a>>,
        selection_level: SelectionLevel,
        transformer: &'c Postgres,
    ) -> Self {
        fn column_path_owned<'a>(
            column_paths: Vec<&ColumnPath<'a>>,
        ) -> Vec<Vec<ColumnPathLink<'a>>> {
            column_paths
                .into_iter()
                .filter_map(|path| match path {
                    ColumnPath::Physical(links) => Some(links.to_vec()),
                    _ => None,
                })
                .collect()
        }

        let predicate_column_paths: Vec<Vec<ColumnPathLink>> =
            column_path_owned(abstract_select.predicate.column_paths());

        let order_by_column_paths = abstract_select
            .order_by
            .as_ref()
            .map(|ob| column_path_owned(ob.column_paths()))
            .unwrap_or_else(Vec::new);

        let columns_paths = predicate_column_paths
            .iter()
            .chain(order_by_column_paths.iter());

        let has_a_one_to_many_clause = columns_paths
            .clone()
            .any(|path| path.iter().any(|link| link.is_one_to_many()));

        let is_return_value_agg = matches!(abstract_select.selection, Selection::Json(..));

        Self {
            abstract_select,
            has_a_one_to_many_clause,
            predicate_column_paths,
            order_by_column_paths,
            additional_predicate,
            selection_level,
            is_return_value_agg,
            transformer,
        }
    }
}
