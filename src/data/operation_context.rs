use async_graphql_value::Value;
use typed_arena::Arena;

use crate::{
    execution::query_context::QueryContext,
    sql::{column::Column, predicate::Predicate, table::SelectionTable},
};

pub struct OperationContext<'a> {
    pub query_context: &'a QueryContext<'a>,
    columns: Arena<Column<'a>>,
    predicates: Arena<Predicate<'a>>,
}

impl<'a> OperationContext<'a> {
    pub fn new(query_context: &'a QueryContext<'a>) -> Self {
        Self {
            query_context,
            columns: Arena::new(),
            predicates: Arena::new(),
        }
    }

    pub fn create_column(&self, column: Column<'a>) -> &Column<'a> {
        self.columns.alloc(column)
    }

    pub fn create_predicate(&self, predicate: Predicate<'a>) -> &Predicate<'a> {
        self.predicates.alloc(predicate)
    }

    pub fn literal_column<'b>(&'b self, value: &'b Value) -> &'b Column<'b> {
        let column = match value {
            Value::Variable(_) => todo!(),
            Value::Number(v) => {
                // TODO: Work with the database schema to cast to appropriate i32/f32, etc types
                Column::Literal(Box::new(v.as_i64().unwrap() as i32))
            }
            Value::String(v) => Column::Literal(Box::new(v.to_owned())),
            Value::Boolean(v) => Column::Literal(Box::new(*v)),
            Value::Null => todo!(),
            Value::Enum(v) => Column::Literal(Box::new(v.to_string())), // We might need guidance from database to do a correct translation
            Value::List(_) => todo!(),
            Value::Object(_) => panic!(),
        };

        self.create_column(column)
    }
}
