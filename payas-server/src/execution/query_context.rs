use std::collections::HashMap;

use anyhow::Result;
use async_graphql_parser::{
    types::{
        BaseType, Field, FragmentDefinition, FragmentSpread, OperationDefinition, OperationType,
        Type,
    },
    Positioned,
};
use async_graphql_value::{Name, Number, Value};
use chrono::prelude::*;
use chrono::DateTime;
use payas_model::{
    model::{column_id::ColumnId, system::ModelSystem},
    sql::{column::*, SQLParam},
};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde_json::{Map, Value as JsonValue};
use typed_arena::Arena;

use super::{executor::Executor, resolver::*};

use crate::{data::data_resolver::DataResolver, introspection::schema::*};

pub struct QueryContext<'a> {
    pub operation_name: Option<&'a str>,
    pub fragment_definitions: HashMap<Name, Positioned<FragmentDefinition>>,
    pub variables: &'a Option<&'a Map<String, JsonValue>>,
    pub executor: &'a Executor<'a>,
    pub request_context: &'a serde_json::Value,
    pub resolved_variables: Arena<Value>,
}

#[derive(Debug, Clone)]
pub enum QueryResponse {
    Json(JsonValue),
    Raw(Option<String>),
}

impl QueryResponse {
    pub fn to_json(&self) -> Result<JsonValue> {
        match &self {
            QueryResponse::Json(val) => Ok(val.clone()),
            QueryResponse::Raw(raw) => {
                if let Some(raw) = raw {
                    Ok(serde_json::from_str(raw)?)
                } else {
                    Ok(JsonValue::Null)
                }
            }
        }
    }
}

impl<'qc> QueryContext<'qc> {
    pub fn resolve_operation<'b>(
        &self,
        operation: (Option<&Name>, &'b Positioned<OperationDefinition>),
    ) -> Result<Vec<(String, QueryResponse)>> {
        operation
            .1
            .node
            .resolve_selection_set(self, &operation.1.node.selection_set)
    }

    pub fn fragment_definition(
        &self,
        fragment: &Positioned<FragmentSpread>,
    ) -> Option<&FragmentDefinition> {
        self.fragment_definitions
            .get(&fragment.node.fragment_name.node)
            .map(|v| &v.node)
    }

    fn resolve_type(&self, field: &Field) -> Result<JsonValue> {
        let type_name = &field
            .arguments
            .iter()
            .find(|arg| arg.0.node == "name")
            .unwrap()
            .1;

        if let Value::String(name_specified) = &type_name.node {
            let tpe: Type = Type {
                base: BaseType::Named(Name::new(name_specified)),
                nullable: true,
            };
            tpe.resolve_value(self, &field.selection_set)
        } else {
            Ok(JsonValue::Null)
        }
    }

    pub fn create_column_with_id(&self, column_id: &ColumnId) -> Column<'qc> {
        Column::Physical(column_id.get_column(self.executor.system))
    }

    pub fn literal_column(
        &'qc self,

        // TODO: we probably don't need to pass around `Value`
        // this was originally `&Value` because we weren't creating any new `Value` objects in this function
        // however, this is now a `Value` since we use `Value::from_json` to parse out a `Value` from JSON variables
        // (this results in a new `Value`!)
        // can we shift value ownership (maybe something like an `Rc<Value>`) to avoid unnecessary clones in
        // data_param_mapper.rs and predicate_mapper.rs ?
        value: Value,
        associated_column: &PhysicalColumn,
    ) -> Column<'qc> {
        match value {
            Value::Variable(name) => {
                let value = self
                    .variables
                    .and_then(|variable| variable.get(name.as_str()))
                    .map(|value| async_graphql_value::Value::from_json(value.clone()).unwrap())
                    .unwrap();

                Self::literal_column(self, value, associated_column)
            }
            Value::Number(number) => Column::Literal(cast_number(&number, &associated_column.typ)),
            Value::String(v) => Column::Literal(cast_string(&v, &associated_column.typ)),
            Value::Boolean(v) => Column::Literal(Box::new(v)),
            Value::Null => Column::Null,
            Value::Enum(v) => Column::Literal(Box::new(v.to_string())), // We might need guidance from the database to do a correct translation
            Value::List(v) => {
                let values = v
                    .into_iter()
                    .map(|elem| Self::literal_column(self, elem, associated_column).into())
                    .collect();

                Column::Array(values)
            }
            Value::Object(_) => Column::Literal(cast_value(&value, &associated_column.typ)),
            Value::Binary(_) => panic!("Binary values are not supported"),
        }
    }

    pub fn resolve_variable(&self, name: &str) -> Option<&Value> {
        let resolved: Option<&serde_json::Value> =
            self.variables.and_then(|variables| variables.get(name));

        resolved.map(|json_value| {
            let value = Value::from_json(json_value.to_owned()).unwrap();
            self.resolved_variables.alloc(value) as &Value
        })
    }

    pub fn get_argument_field(
        &'qc self,
        argument_value: &'qc Value,
        field_name: &str,
    ) -> Option<&'qc Value> {
        match argument_value {
            Value::Object(value) => value.get(field_name),
            Value::Variable(name) => self.resolve_variable(name.as_str()),
            _ => None,
        }
    }

    pub fn get_system(&self) -> &ModelSystem {
        self.executor.system
    }
}

fn cast_number(number: &Number, destination_type: &PhysicalColumnType) -> Box<dyn SQLParam> {
    match destination_type {
        PhysicalColumnType::Int { bits } => match bits {
            IntBits::_16 => Box::new(number.as_i64().unwrap() as i16),
            IntBits::_32 => Box::new(number.as_i64().unwrap() as i32),
            IntBits::_64 => Box::new(number.as_i64().unwrap() as i64),
        },
        PhysicalColumnType::Float { bits } => match bits {
            FloatBits::_24 => Box::new(number.as_f64().unwrap() as f32),
            FloatBits::_53 => Box::new(number.as_f64().unwrap() as f64),
        },
        PhysicalColumnType::Numeric { .. } => {
            let decimal = Decimal::from_str(&number.to_string());
            Box::new(decimal.unwrap())
        }
        PhysicalColumnType::ColumnReference { ref_pk_type, .. } => {
            // TODO assumes that `id` columns are always integers
            cast_number(number, ref_pk_type)
        }
        // TODO: Expand for other number types such as float
        _ => panic!("Unexpected destination_type for number value"),
    }
}

fn cast_string(string: &str, destination_type: &PhysicalColumnType) -> Box<dyn SQLParam> {
    match destination_type {
        PhysicalColumnType::Numeric { .. } => Box::new(Decimal::from_str(string).unwrap()),

        PhysicalColumnType::Timestamp { timezone, .. } => {
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

        PhysicalColumnType::Array { typ } => cast_string(string, typ),

        _ => Box::new(string.to_owned()),
    }
}

fn cast_value(val: &Value, destination_type: &PhysicalColumnType) -> Box<dyn SQLParam> {
    match destination_type {
        PhysicalColumnType::Json => {
            let json_object = val.clone().into_json().unwrap();
            Box::new(json_object)
        }
        _ => panic!(),
    }
}

/**
Go through the FieldResolver (thus through the generic support offered by Resolver) and
so that we can support fragments in top-level queries in such as:

```graphql
{
  ...query_info
}

fragment query_info on Query {
  __type(name: "Concert") {
    name
  }

  __schema {
      types {
      name
    }
  }
}
```
*/
impl FieldResolver<QueryResponse> for OperationDefinition {
    fn resolve_field<'a>(
        &'a self,
        query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<QueryResponse> {
        match field.node.name.node.as_str() {
            "__type" => Ok(QueryResponse::Json(
                query_context.resolve_type(&field.node)?,
            )),
            "__schema" => Ok(QueryResponse::Json(
                query_context
                    .executor
                    .schema
                    .resolve_value(query_context, &field.node.selection_set)?,
            )),
            "__typename" => {
                let typename = match self.ty {
                    OperationType::Query => QUERY_ROOT_TYPENAME,
                    OperationType::Mutation => MUTATION_ROOT_TYPENAME,
                    OperationType::Subscription => SUBSCRIPTION_ROOT_TYPENAME,
                };
                Ok(QueryResponse::Json(JsonValue::String(typename.to_string())))
            }
            _ => query_context
                .executor
                .system
                .resolve(field, &self.ty, query_context),
        }
    }
}
