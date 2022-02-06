use crate::sql::order::{OrderBy, Ordering};

use super::column_path::ColumnPath;

#[derive(Debug)]
pub struct AbstractOrderBy<'a>(pub Vec<(ColumnPath<'a>, Ordering)>);

impl<'a> AbstractOrderBy<'a> {
    pub fn order_by(&'a self) -> OrderBy<'a> {
        OrderBy(
            self.0
                .iter()
                .map(|(path, ordering)| (path.leaf_column(), ordering.clone()))
                .collect(),
        )
    }

    pub fn column_paths(&'a self) -> Vec<&'a ColumnPath<'a>> {
        self.0.iter().map(|(path, _)| path).collect()
    }
}
