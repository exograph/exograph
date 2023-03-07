use super::{
    column::Column, delete::Delete, insert::Insert, physical_column::PhysicalColumn,
    predicate::ConcretePredicate, update::Update, ExpressionBuilder,
};

use maybe_owned::MaybeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PhysicalTable {
    pub name: String,
    pub columns: Vec<PhysicalColumn>,
}

impl std::fmt::Debug for PhysicalTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Table: ")?;
        f.write_str(&self.name)
    }
}

impl PhysicalTable {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.column_name == name)
    }

    pub fn get_column(&self, name: &str) -> Option<Column> {
        self.get_physical_column(name).map(Column::Physical)
    }

    pub fn get_physical_column(&self, name: &str) -> Option<&PhysicalColumn> {
        self.columns
            .iter()
            .find(|column| column.column_name == name)
    }

    pub fn get_pk_physical_column(&self) -> Option<&PhysicalColumn> {
        self.columns.iter().find(|column| column.is_pk)
    }

    pub fn insert<'a, C>(
        &'a self,
        column_names: Vec<&'a PhysicalColumn>,
        column_values_seq: Vec<Vec<C>>,
        returning: Vec<MaybeOwned<'a, Column<'a>>>,
    ) -> Insert
    where
        C: Into<MaybeOwned<'a, Column<'a>>>,
    {
        Insert {
            table: self,
            column_names,
            column_values_seq: column_values_seq
                .into_iter()
                .map(|rows| rows.into_iter().map(|col| col.into()).collect())
                .collect(),
            returning,
        }
    }

    pub fn delete<'a>(
        &'a self,
        predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
        returning: Vec<MaybeOwned<'a, Column<'a>>>,
    ) -> Delete {
        Delete {
            table: self,
            predicate,
            returning,
        }
    }

    pub fn update<'a, C>(
        &'a self,
        column_values: Vec<(&'a PhysicalColumn, C)>,
        predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
        returning: Vec<MaybeOwned<'a, Column<'a>>>,
    ) -> Update
    where
        C: Into<MaybeOwned<'a, Column<'a>>>,
    {
        Update {
            table: self,
            column_values: column_values
                .into_iter()
                .map(|(pc, col)| (pc, col.into()))
                .collect(),
            predicate,
            returning,
        }
    }
}

impl ExpressionBuilder for PhysicalTable {
    fn build(&self, builder: &mut crate::sql::SQLBuilder) {
        builder.push_identifier(&self.name);
    }
}
