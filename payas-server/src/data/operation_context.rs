use async_graphql_value::{from_value, Number, Value};
use chrono::offset::TimeZone;
use chrono::prelude::*;
use payas_model::{model::column_id::ColumnId, sql::column::IntBits};
use std::collections::BTreeMap;
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
            Value::Object(object) => {
                Column::Literal(Self::cast_object(&object, &associated_column.typ))
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

    fn cast_object(
        obj: &BTreeMap<async_graphql_value::Name, async_graphql_value::Value>,
        destination_type: &PhysicalColumnType,
    ) -> Box<dyn SQLParam> {
        let get_u32 = |s: &str| -> u32 {
            from_value(obj.get(s).unwrap().clone().into_const().unwrap()).unwrap()
        };
        let get_i32 = |s: &str| -> i32 {
            from_value(obj.get(s).unwrap().clone().into_const().unwrap()).unwrap()
        };

        match destination_type {
            PhysicalColumnType::Timestamp { .. } | PhysicalColumnType::Time { .. } => {
                let hours = get_u32("hours");
                let minutes = get_u32("minutes");
                let seconds = get_u32("seconds");
                let nanoseconds = get_u32("ns");
                let t = NaiveTime::from_hms_nano(hours, minutes, seconds, nanoseconds);

                if let PhysicalColumnType::Timestamp { timezone, .. } = destination_type {
                    let year = get_i32("year");
                    let month = get_u32("month");
                    let day = get_u32("day");

                    let d = NaiveDate::from_ymd(year, month, day);
                    let dt = NaiveDateTime::new(d, t);

                    if *timezone {
                        let offset = get_i32("offset");
                        Box::new(FixedOffset::east(offset).from_local_datetime(&dt).unwrap())
                    } else {
                        Box::new(dt)
                    }
                } else if let PhysicalColumnType::Time { .. } = destination_type {
                    Box::new(t)
                } else {
                    panic!()
                }
            }

            PhysicalColumnType::Date => {
                let year = get_i32("year");
                let month = get_u32("month");
                let day = get_u32("day");

                Box::new(NaiveDate::from_ymd(year, month, day))
            }

            _ => panic!(),
        }
    }
}
