use async_graphql_value::{Number, Value};
use payas_model::{model::column_id::ColumnId, sql::column::IntBits};
use typed_arena::Arena;

use crate::{
    execution::query_context::QueryContext,
    sql::{
        column::{Column, PhysicalColumn, PhysicalColumnType},
        predicate::Predicate,
        SQLParam,
    },
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

    pub fn create_column_with_id(&self, column_id: &ColumnId) -> &Column<'a> {
        self.create_column(Column::Physical(
            column_id.get_column(self.query_context.system),
        ))
    }

    pub fn create_predicate(&self, predicate: Predicate<'a>) -> &Predicate<'a> {
        self.predicates.alloc(predicate)
    }

    pub fn literal_column<'b>(
        &'b self,
        value: &'b Value,
        associated_column: &PhysicalColumn,
    ) -> &'b Column<'b> {
        let column = match value {
            Value::Variable(name) => {
                let value = self
                    .query_context
                    .variables
                    .and_then(|variable| variable.get(name.as_str()))
                    .unwrap();
                Column::Literal(Self::cast_value(value, &associated_column.typ))
            }
            Value::Number(number) => {
                Column::Literal(Self::cast_number(number, &associated_column.typ))
            }
            Value::String(v) => Column::Literal(Box::new(v.to_owned())),
            Value::Boolean(v) => Column::Literal(Box::new(*v)),
            Value::Null => Column::Null,
            Value::Enum(v) => Column::Literal(Box::new(v.to_string())), // We might need guidance from database to do a correct translation
            Value::List(_) => todo!(),
            Value::Object(_) => panic!(),
        };

        self.create_column(column)
    }

    fn cast_value(
        value: &serde_json::value::Value,
        destination_type: &PhysicalColumnType,
    ) -> Box<dyn SQLParam> {
        match destination_type {
            PhysicalColumnType::Int { bits } => match bits {
                IntBits::_16 => Box::new(value.as_i64().unwrap() as i16),
                IntBits::_32 => Box::new(value.as_i64().unwrap() as i32),
                IntBits::_64 => Box::new(value.as_i64().unwrap() as i64),
            },
            PhysicalColumnType::String => Box::new(value.as_str().unwrap().to_string()),
            PhysicalColumnType::Boolean => Box::new(value.as_bool().unwrap()),
        }
    }

    fn cast_number(number: &Number, destination_type: &PhysicalColumnType) -> Box<dyn SQLParam> {
        match destination_type {
            PhysicalColumnType::Int { bits } => match bits {
                IntBits::_16 => Box::new(number.as_i64().unwrap() as i16),
                IntBits::_32 => Box::new(number.as_i64().unwrap() as i32),
                IntBits::_64 => Box::new(number.as_i64().unwrap() as i64),
            },
            // TODO: Expand for other number types such as float
            _ => panic!("Unexpected destination_type for number value"),
        }
    }
}
