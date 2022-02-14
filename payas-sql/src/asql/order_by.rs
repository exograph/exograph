use crate::sql::order::{OrderBy, Ordering};

use super::column_path::ColumnPath;

#[derive(Debug)]
pub struct AbstractOrderBy<'a>(pub Vec<(ColumnPath<'a>, Ordering)>);

impl<'a> AbstractOrderBy<'a> {
    pub fn order_by(self) -> OrderBy<'a> {
        OrderBy(
            self.0
                .into_iter()
                .map(|(path, ordering)| (path.leaf_column(), ordering))
                .collect(),
        )
    }

    pub fn column_paths(&self) -> Vec<&ColumnPath<'a>> {
        self.0.iter().map(|(path, _)| path).collect()
    }
}
