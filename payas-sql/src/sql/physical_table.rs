use super::{
    column::{Column, PhysicalColumn},
    order::OrderBy,
    predicate::Predicate,
    select::Select,
    Delete, Expression, ExpressionContext, Insert, ParameterBinding, Update,
};

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalTable {
    pub name: String,
    pub columns: Vec<PhysicalColumn>,
}

impl PhysicalTable {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.column_name == name)
    }

    pub fn get_column(&self, name: &str) -> Option<Column> {
        self.columns
            .iter()
            .find(|column| column.column_name == name)
            .map(|physical_column| Column::Physical(physical_column))
    }

    pub fn select<'a>(
        &'a self,
        columns: Vec<&'a Column>,
        predicate: Option<&'a Predicate<'a>>,
        order_by: Option<OrderBy<'a>>,
        top_level_selection: bool,
    ) -> Select {
        Select {
            underlying: self,
            columns,
            predicate,
            order_by,
            top_level_selection,
        }
    }

    pub fn insert<'a>(
        &'a self,
        column_values: Vec<(&'a Column<'a>, &'a Column<'a>)>,
        returning: Vec<&'a Column>,
    ) -> Insert {
        Insert {
            table: self,
            column_values,
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

    pub fn update<'a>(
        &'a self,
        column_values: Vec<(&'a Column<'a>, &'a Column<'a>)>,
        predicate: &'a Predicate<'a>,
        returning: Vec<&'a Column>,
    ) -> Update {
        Update {
            table: self,
            column_values,
            predicate,
            returning,
        }
    }
}

impl Expression for PhysicalTable {
    fn binding(&self, _expression_context: &mut ExpressionContext) -> ParameterBinding {
        ParameterBinding::new(format!(r#""{}""#, self.name.clone()), vec![])
    }
}
