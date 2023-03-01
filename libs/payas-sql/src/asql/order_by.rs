use crate::sql::{
    column::PhysicalColumn,
    order::{OrderBy, Ordering},
};

use super::column_path::ColumnPath;

#[derive(Debug)]
pub struct AbstractOrderBy<'a>(pub Vec<(ColumnPath<'a>, Ordering)>);

impl<'a> AbstractOrderBy<'a> {
    pub fn leaf_column(column_path: &ColumnPath<'a>) -> &'a PhysicalColumn {
        match column_path {
            ColumnPath::Physical(links) => links.last().unwrap().self_column.0,
            _ => panic!("Cannot get leaf column from literal or null"),
        }
    }

    pub fn order_by(&self) -> OrderBy<'a> {
        OrderBy(
            self.0
                .iter()
                .map(|(path, ordering)| (Self::leaf_column(path), *ordering))
                .collect(),
        )
    }

    pub fn column_paths(&self) -> Vec<&ColumnPath<'a>> {
        self.0.iter().map(|(path, _)| path).collect()
    }
}
