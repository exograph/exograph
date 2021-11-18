use std::collections::HashMap;

use anyhow::Result;
use async_graphql_parser::{
    types::{
        BaseType, Field, FragmentDefinition, FragmentSpread, OperationDefinition, OperationType,
        Type,
    },
    Positioned,
};
use async_graphql_value::{ConstValue, Name, Number, Value};
use chrono::prelude::*;
use chrono::DateTime;
use payas_model::{
    model::{column_id::ColumnId, system::ModelSystem},
    sql::{column::*, SQLBytes, SQLParam},
};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde_json::{Map, Value as JsonValue};
use typed_arena::Arena;

use super::{executor::Executor, resolver::*};

use crate::{data::data_resolver::DataResolver, error::ExecutionError, introspection::schema::*};

pub struct QueryContext<'a> {
    pub operation_name: Option<&'a str>,
    pub fragment_definitions: HashMap<Name, Positioned<FragmentDefinition>>,
    pub variables: &'a Option<&'a Map<String, JsonValue>>,
    pub executor: &'a Executor<'a>,
    pub request_context: &'a serde_json::Value,
    pub field_arguments: Arena<Vec<(Positioned<Name>, Positioned<ConstValue>)>>,
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

    // TODO: currently just unwrapping the result when we call this method from somewhere
    // take a look at how to properly handle errors from this...
    pub fn literal_column(
        &'qc self,
        value: &ConstValue,
        associated_column: &PhysicalColumn,
    ) -> Result<Column<'qc>> {
        let value = match value {
            ConstValue::Number(number) => {
                Column::Literal(cast_number(number, &associated_column.typ))
            }
            ConstValue::String(v) => Column::Literal(cast_string(v, &associated_column.typ)?),
            ConstValue::Boolean(v) => Column::Literal(Box::new(*v)),
            ConstValue::Null => Column::Null,
            ConstValue::Enum(v) => Column::Literal(Box::new(v.to_string())), // We might need guidance from the database to do a correct translation
            ConstValue::List(v) => {
                let values = v
                    .iter()
                    .map(|elem| self.literal_column(elem, associated_column))
                    .collect::<Result<Vec<_>>>()?;

                let deref = values.into_iter().map(Into::into).collect();

                Column::Array(deref)
            }
            ConstValue::Object(_) => Column::Literal(cast_value(value, &associated_column.typ)),
            ConstValue::Binary(bytes) => Column::Literal(Box::new(SQLBytes(bytes.clone()))),
        };

        Ok(value)
    }

    pub fn field_arguments(
        &'qc self,
        field: &Field,
    ) -> Result<&'qc Vec<(Positioned<Name>, Positioned<ConstValue>)>> {
        let args: Result<Vec<(Positioned<Name>, Positioned<ConstValue>)>> = field
            .arguments
            .iter()
            .map(|(name, value)| {
                let v = value
                    .node
                    .clone()
                    .into_const_with(|_| self.var_value(name))?;

                Ok((name.clone(), Positioned::new(v, value.pos)))
            })
            .collect();

        Ok(self.field_arguments.alloc(args?))
    }

    fn var_value(&self, name: &Positioned<Name>) -> Result<ConstValue, ExecutionError> {
        let resolved: Option<&serde_json::Value> = self
            .variables
            .and_then(|variables| variables.get(name.node.as_str()));

        resolved
            .map(|json_value| ConstValue::from_json(json_value.to_owned()).unwrap())
            .ok_or_else(|| {
                ExecutionError::VariableNotFound(name.node.as_str().to_string(), name.pos)
            })
    }

    pub fn get_argument_field(
        &'qc self,
        argument_value: &'qc ConstValue,
        field_name: &str,
    ) -> Option<&'qc ConstValue> {
        match argument_value {
            ConstValue::Object(value) => value.get(field_name),
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

fn cast_string(string: &str, destination_type: &PhysicalColumnType) -> Result<Box<dyn SQLParam>> {
    let value: Box<dyn SQLParam> = match destination_type {
        PhysicalColumnType::Numeric { .. } => Box::new(Decimal::from_str(string)?),

        PhysicalColumnType::Timestamp { timezone, .. } => {
            if *timezone {
                let dt = DateTime::parse_from_rfc3339(string)?;
                Box::new(dt)
            } else {
                let dt = NaiveDateTime::parse_from_str(string, "%Y-%m-%dT%H:%M:%S%.f")?;
                Box::new(dt)
            }
        }

        PhysicalColumnType::Time { .. } => {
            let t = NaiveTime::parse_from_str(string, "%H:%M:%S%.f")?;
            Box::new(t)
        }

        PhysicalColumnType::Date => {
            let d = NaiveDate::parse_from_str(string, "%Y-%m-%d")?;
            Box::new(d)
        }

        PhysicalColumnType::Blob => {
            let bytes = base64::decode(string)?;
            Box::new(SQLBytes::new(bytes))
        }

        PhysicalColumnType::Array { typ } => cast_string(string, typ)?,

        _ => Box::new(string.to_owned()),
    };

    Ok(value)
}

fn cast_value(val: &ConstValue, destination_type: &PhysicalColumnType) -> Box<dyn SQLParam> {
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
