use async_graphql_value::{Number, Value};
use chrono::prelude::*;
use chrono::DateTime;
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
    resolved_variables: Arena<Value>,
}

impl<'a> OperationContext<'a> {
    pub fn new(query_context: &'a QueryContext<'a>) -> Self {
        Self {
            query_context,
            columns: Arena::new(),
            predicates: Arena::new(),
            resolved_variables: Arena::new(),
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

    pub fn literal_column(
        &'a self,
        value: &'a Value,
        associated_column: &PhysicalColumn,
    ) -> &'a Column<'a> {
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
            Value::String(v) => Column::Literal(Self::cast_string(v, &associated_column.typ)),
            Value::Boolean(v) => Column::Literal(Box::new(*v)),
            Value::Null => Column::Null,
            Value::Enum(v) => Column::Literal(Box::new(v.to_string())), // We might need guidance from database to do a correct translation
            Value::List(v) => {
                let values: Vec<&'a Column<'a>> = v
                    .iter()
                    .map(|x| Self::literal_column(self, &x, associated_column))
                    .collect();

                Column::Array(values)
            }
            Value::Object(_) => {
                panic!()
            }
        };

        self.create_column(column)
    }

    pub fn resolve_variable(&self, name: &str) -> Option<&Value> {
        let resolved: Option<&serde_json::Value> = self
            .query_context
            .variables
            .and_then(|variables| variables.get(name));

        resolved.map(|json_value| {
            let value = Value::from_json(json_value.to_owned()).unwrap();
            let non_mut_value: &Value = self.resolved_variables.alloc(value);
            non_mut_value
        })
    }

    pub fn get_argument_field(
        &'a self,
        argument_value: &'a Value,
        field_name: &str,
    ) -> Option<&'a Value> {
        match argument_value {
            Value::Object(value) => value.get(field_name),
            Value::Variable(name) => self.resolve_variable(name.as_str()),
            _ => None,
        }
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
            PhysicalColumnType::String { length: _ } => {
                Box::new(value.as_str().unwrap().to_string())
            }
            PhysicalColumnType::Boolean => Box::new(value.as_bool().unwrap()),
            PhysicalColumnType::Timestamp { .. } => panic!(),
            PhysicalColumnType::Date => panic!(),
            PhysicalColumnType::Time { .. } => panic!(),
            PhysicalColumnType::Array { .. } => panic!(),
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

    fn cast_string(string: &str, destination_type: &PhysicalColumnType) -> Box<dyn SQLParam> {
        match destination_type {
            PhysicalColumnType::Timestamp { timezone, .. } => {
                println!("{} {:?}", string, destination_type);

                if *timezone {
                    let dt = DateTime::parse_from_rfc3339(string).unwrap();
                    Box::new(dt)
                } else {
                    let dt = NaiveDateTime::parse_from_str(string, "%Y-%m-%dT%H:%M:%S%.f").unwrap();
                    Box::new(dt)
                }
            }

            PhysicalColumnType::Time { .. } => {
                let t = NaiveTime::parse_from_str(string, "%H:%M:%S%.f").unwrap();
                Box::new(t)
            }

            PhysicalColumnType::Date => {
                let d = NaiveDate::parse_from_str(string, "%Y-%m-%d").unwrap();
                Box::new(d)
            }

            PhysicalColumnType::String { .. } => Box::new(string.to_owned()),

            PhysicalColumnType::Array { typ } => Self::cast_string(string, typ),

            _ => panic!(),
        }
    }
}
