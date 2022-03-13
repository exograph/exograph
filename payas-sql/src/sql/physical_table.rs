use crate::spec::TableSpec;

use super::{
    column::{Column, PhysicalColumn},
    delete::Delete,
    insert::Insert,
    predicate::Predicate,
    update::Update,
    Expression, ExpressionContext, ParameterBinding,
};

use maybe_owned::MaybeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct PhysicalTable {
    pub name: String,
    pub columns: Vec<PhysicalColumn>,
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
        predicate: MaybeOwned<'a, Predicate<'a>>,
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
        predicate: MaybeOwned<'a, Predicate<'a>>,
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

impl From<TableSpec> for PhysicalTable {
    fn from(t: TableSpec) -> Self {
        Self {
            name: t.name,
            columns: t.column_specs.into_iter().map(|spec| spec.into()).collect(),
        }
    }
}

impl Expression for PhysicalTable {
    fn binding(&self, _expression_context: &mut ExpressionContext) -> ParameterBinding {
        ParameterBinding::new(format!(r#""{}""#, self.name.clone()), vec![])
    }
}
