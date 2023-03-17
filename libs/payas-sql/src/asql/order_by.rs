use crate::sql::order::Ordering;

use super::column_path::ColumnPath;

#[derive(Debug)]
pub struct AbstractOrderBy<'a>(pub Vec<(ColumnPath<'a>, Ordering)>);

impl<'a> AbstractOrderBy<'a> {
    pub fn column_paths(&self) -> Vec<&ColumnPath<'a>> {
        self.0.iter().map(|(path, _)| path).collect()
    }
}
