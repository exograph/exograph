use maybe_owned::MaybeOwned;

use crate::sql::{
    column::{Column, PhysicalColumn},
    predicate::Predicate,
    PhysicalTable,
};

use super::select::{AbstractSelect, SelectionLevel};

#[derive(Debug)]
pub struct ColumnSelection<'a> {
    alias: String,
    column: SelectionElement<'a>,
}

impl<'a> ColumnSelection<'a> {
    pub fn new(alias: String, column: SelectionElement<'a>) -> Self {
        Self { alias, column }
    }
}

#[derive(Debug)]
pub enum SelectionCardinality {
    One,
    Many,
}

#[derive(Debug)]
pub enum Selection<'a> {
    Seq(Vec<ColumnSelection<'a>>),
    Json(Vec<ColumnSelection<'a>>, SelectionCardinality),
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
                            SelectionElement::Constant(s) => Column::Constant(s.to_owned()),
                            SelectionElement::Nested(_, _) => {
                                panic!("Nested selection not supported in Selection::Seq")
                            }
                        },
                    )
                    .collect(),
            ),
            Selection::Json(seq, cardinality) => {
                let object_elems = seq
                    .iter()
                    .map(|ColumnSelection { alias, column }| (alias.clone(), column.to_sql()))
                    .collect();

                let json_obj = Column::JsonObject(object_elems);

                match cardinality {
                    SelectionCardinality::One => SelectionSQL::Single(json_obj),
                    SelectionCardinality::Many => {
                        SelectionSQL::Single(Column::JsonAgg(Box::new(json_obj.into())))
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum SelectionElement<'a> {
    Physical(&'a PhysicalColumn),
    Constant(String), // To support __typename
    Nested(NestedElementRelation<'a>, AbstractSelect<'a>),
}

/// Relation between two tables
/// The `column` is the column in the one table that is joined to the other `table`('s primary key)
/// TODO: Could this idea be consolidated with the `ColumnPath`? After all, both represent a way to link two tables
#[derive(Debug)]
pub struct NestedElementRelation<'a> {
    pub column: &'a PhysicalColumn,
    pub table: &'a PhysicalTable,
}

impl<'a> NestedElementRelation<'a> {
    pub fn new(column: &'a PhysicalColumn, table: &'a PhysicalTable) -> Self {
        Self { column, table }
    }
}

impl<'a> SelectionElement<'a> {
    pub fn to_sql(&'a self) -> MaybeOwned<'a, Column<'a>> {
        match self {
            SelectionElement::Physical(pc) => Column::Physical(pc),
            SelectionElement::Constant(s) => Column::Constant(s.clone()),
            SelectionElement::Nested(relation, select) => {
                Column::SelectionTableWrapper(Box::new(select.to_select(
                    Some(Predicate::Eq(
                        Column::Physical(relation.column).into(),
                        Column::Physical(relation.table.get_pk_physical_column().unwrap()).into(),
                    )),
                    SelectionLevel::Nested,
                )))
            }
        }
        .into()
    }
}
