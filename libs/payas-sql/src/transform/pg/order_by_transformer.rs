use crate::{
    sql::order::{OrderBy, OrderByElement},
    transform::transformer::OrderByTransformer,
    AbstractOrderBy, ColumnPath, PhysicalColumn,
};

use super::Postgres;

impl OrderByTransformer for Postgres {
    fn to_order_by<'a>(&self, order_by: &AbstractOrderBy<'a>) -> OrderBy<'a> {
        OrderBy(
            order_by
                .0
                .iter()
                .map(|(path, ordering)| OrderByElement::new(leaf_column(path), *ordering))
                .collect(),
        )
    }
}

fn leaf_column<'a>(column_path: &ColumnPath<'a>) -> &'a PhysicalColumn {
    match column_path {
        ColumnPath::Physical(links) => links.last().unwrap().self_column.0,
        _ => panic!("Cannot get leaf column from literal or null"),
    }
}
