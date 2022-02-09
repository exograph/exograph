use crate::sql::{
    column::{Column, PhysicalColumn},
    predicate::Predicate,
    PhysicalTable,
};

use super::select::AbstractSelect;

#[derive(Debug)]
pub struct ColumnSelection<'a> {
    alias: &'a str,
    column: SelectionElement<'a>,
}

impl<'a> ColumnSelection<'a> {
    pub fn new(alias: &'a str, column: SelectionElement<'a>) -> Self {
        Self { alias, column }
    }
}

#[derive(Debug)]
pub enum Selection<'a> {
    Seq(Vec<ColumnSelection<'a>>),
    Json(Vec<ColumnSelection<'a>>, bool),
}

pub enum SelectionSQL<'a> {
    Single(Column<'a>),
    Seq(Vec<Column<'a>>),
}

impl<'a> Selection<'a> {
    pub fn to_sql(&'a self) -> SelectionSQL<'a> {
        match self {
            Selection::Seq(seq) => SelectionSQL::Seq(
                seq.iter()
                    .map(
                        |ColumnSelection {
                             alias: _alias,
                             column,
                         }| match column {
                            // TODO: Support alias (requires a change to `Select`)
                            SelectionElement::Physical(pc) => Column::Physical(pc),
                            SelectionElement::Nested(_, _) => todo!(),
                        },
                    )
                    .collect(),
            ),
            Selection::Json(seq, multi) => {
                let object_elems = seq
                    .iter()
                    .map(|ColumnSelection { alias, column }| {
                        (alias.to_string(), column.to_sql().into())
                    })
                    .collect();

                let json_obj = Column::JsonObject(object_elems);

                if *multi {
                    SelectionSQL::Single(Column::JsonAgg(Box::new(json_obj.into())))
                } else {
                    SelectionSQL::Single(json_obj)
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum SelectionElement<'a> {
    Physical(&'a PhysicalColumn),
    Nested(SelectionElementRelation<'a>, AbstractSelect<'a>),
}

/// Relation between two tables
/// The `column` is the column in the one table that is joined to the other `table`('s primary key)
/// TODO: Could this idea be consolidated with the `ColumnPath`? After all, both represent a way to link two tables
#[derive(Debug)]
pub struct SelectionElementRelation<'a> {
    pub column: &'a PhysicalColumn,
    pub table: &'a PhysicalTable,
}

impl<'a> SelectionElementRelation<'a> {
    pub fn new(column: &'a PhysicalColumn, table: &'a PhysicalTable) -> Self {
        Self { column, table }
    }
}

impl<'a> SelectionElement<'a> {
    pub fn to_sql(&'a self) -> Column<'a> {
        match self {
            SelectionElement::Physical(pc) => Column::Physical(pc),
            SelectionElement::Nested(relation, select) => {
                Column::SelectionTableWrapper(Box::new(select.to_sql(
                    Some(Predicate::Eq(
                        Column::Physical(relation.column).into(),
                        Column::Physical(relation.table.get_pk_physical_column().unwrap()).into(),
                    )),
                    false,
                )))
            }
        }
    }
}
