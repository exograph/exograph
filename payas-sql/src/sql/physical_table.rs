use crate::spec::TableSpec;

use super::{
    column::{Column, PhysicalColumn},
    limit::Limit,
    offset::Offset,
    order::OrderBy,
    predicate::Predicate,
    select::Select,
    Delete, Expression, ExpressionContext, Insert, ParameterBinding, Update,
};

use maybe_owned::MaybeOwned;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PhysicalTable {
    pub name: String,
    pub columns: Vec<PhysicalColumn>,
}

impl PhysicalTable {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.column_name == name)
    }

    pub fn get_column(&self, name: &str) -> Option<Column> {
        self.get_physical_column(name)
            .map(|physical_column| Column::Physical(physical_column))
    }

    pub fn get_physical_column(&self, name: &str) -> Option<&PhysicalColumn> {
        self.columns
            .iter()
            .find(|column| column.column_name == name)
    }

    pub fn get_pk_physical_column(&self) -> Option<&PhysicalColumn> {
        self.columns.iter().find(|column| column.is_pk)
    }

    pub fn select<'a>(
        &'a self,
        columns: Vec<&'a Column>,
        predicate: Option<&'a Predicate<'a>>,
        order_by: Option<OrderBy<'a>>,
        offset: Option<Offset>,
        limit: Option<Limit>,
        top_level_selection: bool,
    ) -> Select {
        Select {
            underlying: self,
            columns,
            predicate,
            order_by,
            offset,
            limit,
            top_level_selection,
        }
    }

    pub fn insert<'a, C>(
        &'a self,
        column_names: Vec<&'a PhysicalColumn>,
        column_values_seq: Vec<Vec<C>>,
        returning: Vec<&'a Column>,
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
        predicate: Option<&'a Predicate<'a>>,
        returning: Vec<&'a Column>,
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
        predicate: &'a Predicate<'a>,
        returning: Vec<&'a Column>,
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
