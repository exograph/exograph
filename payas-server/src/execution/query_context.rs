use std::collections::HashMap;

use anyhow::{bail, Context, Result};
use async_graphql_parser::{
    types::{
        BaseType, Field, FragmentDefinition, FragmentSpread, OperationDefinition, OperationType,
        Type,
    },
    Positioned,
};
use async_graphql_value::{ConstValue, Name, Number, Value};
use async_trait::async_trait;
use chrono::prelude::*;
use chrono::DateTime;
use payas_model::{
    model::{column_id::ColumnId, system::ModelSystem},
    sql::{
        array_util::{self, ArrayEntry},
        column::*,
        SQLBytes, SQLParam,
    },
};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde_json::{Map, Value as JsonValue};
use typed_arena::Arena;

use super::{executor::Executor, resolver::*};

use crate::{data::data_resolver::DataResolver, error::ExecutionError, introspection::schema::*};

const NAIVE_DATE_FORMAT: &str = "%Y-%m-%d";
const NAIVE_TIME_FORMAT: &str = "%H:%M:%S%.f";

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
    pub async fn resolve_operation<'b>(
        &self,
        operation: (Option<&Name>, &'b Positioned<OperationDefinition>),
    ) -> Result<Vec<(String, QueryResponse)>> {
        operation
            .1
            .node
            .resolve_selection_set(self, &operation.1.node.selection_set)
            .await
    }

    pub fn fragment_definition(
        &self,
        fragment: &Positioned<FragmentSpread>,
    ) -> Result<&FragmentDefinition, ExecutionError> {
        self.fragment_definitions
            .get(&fragment.node.fragment_name.node)
            .map(|v| &v.node)
            .ok_or_else(|| {
                ExecutionError::FragmentDefinitionNotFound(
                    fragment.node.fragment_name.node.as_str().to_string(),
                    fragment.pos,
                )
            })
    }

    async fn resolve_type(&self, field: &Field) -> Result<JsonValue> {
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
            tpe.resolve_value(self, &field.selection_set).await
        } else {
            Ok(JsonValue::Null)
        }
    }

    pub fn create_column_with_id(&self, column_id: &ColumnId) -> Column<'qc> {
        self.executor.system.create_column_with_id(column_id)
    }

    pub fn literal_column(
        &'qc self,
        value: &ConstValue,
        associated_column: &PhysicalColumn,
    ) -> Result<Column<'qc>> {
        cast_value(value, &associated_column.typ)
            .map(|value| value.map(Column::Literal).unwrap_or(Column::Null))
    }

    pub fn field_arguments(
        &'qc self,
        field: &Field,
    ) -> Result<&'qc Vec<(Positioned<Name>, Positioned<ConstValue>)>> {
        let args: Result<Vec<(Positioned<Name>, Positioned<ConstValue>)>> = field
            .arguments
            .iter()
            .map(|(name, value)| {
                let v = value.node.clone().into_const_with(|var_name| {
                    self.var_value(&Positioned::new(var_name, name.pos))
                })?;

                Ok((name.clone(), Positioned::new(v, value.pos)))
            })
            .collect();

        Ok(self.field_arguments.alloc(args?))
    }

    fn var_value(&self, name: &Positioned<Name>) -> Result<ConstValue, ExecutionError> {
        let resolved = self
            .variables
            .and_then(|variables| variables.get(name.node.as_str()))
            .ok_or_else(|| {
                ExecutionError::VariableNotFound(name.node.as_str().to_string(), name.pos)
            })?;

        ConstValue::from_json(resolved.to_owned()).map_err(|e| {
            ExecutionError::MalformedVariable(name.node.as_str().to_string(), name.pos, e)
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

fn cast_value(
    value: &ConstValue,
    destination_type: &PhysicalColumnType,
) -> Result<Option<Box<dyn SQLParam>>> {
    match value {
        ConstValue::Number(number) => Ok(Some(cast_number(number, destination_type))),
        ConstValue::String(v) => cast_string(v, destination_type).map(Some),
        ConstValue::Boolean(v) => Ok(Some(Box::new(*v))),
        ConstValue::Null => Ok(None),
        ConstValue::Enum(v) => Ok(Some(Box::new(v.to_string()))), // We might need guidance from the database to do a correct translation
        ConstValue::List(elems) => cast_list(elems, destination_type),
        ConstValue::Object(_) => Ok(Some(cast_object(value, destination_type))),
        ConstValue::Binary(bytes) => Ok(Some(Box::new(SQLBytes(bytes.clone())))),
    }
}

fn cast_list(
    elems: &[ConstValue],
    destination_type: &PhysicalColumnType,
) -> Result<Option<Box<dyn SQLParam>>> {
    fn array_entry(elem: &ConstValue) -> ArrayEntry<ConstValue> {
        match elem {
            ConstValue::List(elems) => ArrayEntry::List(elems),
            _ => ArrayEntry::Single(elem),
        }
    }

    array_util::to_sql_param(elems, destination_type, array_entry, cast_value)
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

        PhysicalColumnType::Timestamp { .. }
        | PhysicalColumnType::Time { .. }
        | PhysicalColumnType::Date => {
            let datetime = DateTime::parse_from_rfc3339(string);
            let naive_datetime = NaiveDateTime::parse_from_str(
                string,
                &format!("{}T{}", NAIVE_DATE_FORMAT, NAIVE_TIME_FORMAT),
            );

            // attempt to parse string as either datetime+offset or as a naive datetime
            match (datetime, naive_datetime) {
                (Ok(datetime), _) => {
                    match &destination_type {
                        PhysicalColumnType::Timestamp { timezone, .. } => {
                            if *timezone {
                                Box::new(datetime)
                            } else {
                                // default to the naive time if this is a non-timezone field
                                Box::new(datetime.naive_local())
                            }
                        }
                        PhysicalColumnType::Time { .. } => Box::new(datetime.time()),
                        PhysicalColumnType::Date => Box::new(datetime.date().naive_local()),
                        _ => bail!("missing case for datetime in inner match"),
                    }
                }

                (_, Ok(naive_datetime)) => {
                    match &destination_type {
                        PhysicalColumnType::Timestamp { timezone, .. } => {
                            if *timezone {
                                // default to UTC+0 if this field is a timestamp+timezone field
                                Box::new(DateTime::<Utc>::from_utc(naive_datetime, chrono::Utc))
                            } else {
                                Box::new(naive_datetime)
                            }
                        }
                        PhysicalColumnType::Time { .. } => Box::new(naive_datetime.time()),
                        PhysicalColumnType::Date { .. } => Box::new(naive_datetime.date()),
                        _ => bail!("missing case for datetime in inner match"),
                    }
                }

                (Err(_), Err(_)) => {
                    match &destination_type {
                        PhysicalColumnType::Timestamp { .. } => {
                            // exhausted options for timestamp formats
                            bail!("Could not parse {} as a valid timestamp format", string)
                        }
                        PhysicalColumnType::Time { .. } => {
                            // try parsing the string as a time only
                            let t = NaiveTime::parse_from_str(string, NAIVE_TIME_FORMAT)
                                .with_context(|| {
                                    format!(
                                        "Could not parse {} as a valid time-only format",
                                        string
                                    )
                                })?;
                            Box::new(t)
                        }
                        PhysicalColumnType::Date => {
                            // try parsing the string as a date only
                            let d = NaiveDate::parse_from_str(string, NAIVE_DATE_FORMAT)
                                .with_context(|| {
                                    format!(
                                        "Could not parse {} as a valid date-only format",
                                        string
                                    )
                                })?;
                            Box::new(d)
                        }
                        _ => panic!(),
                    }
                }
            }
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

fn cast_object(val: &ConstValue, destination_type: &PhysicalColumnType) -> Box<dyn SQLParam> {
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
#[async_trait(?Send)]
impl FieldResolver<QueryResponse> for OperationDefinition {
    async fn resolve_field<'e>(
        &'e self,
        query_context: &'e QueryContext<'e>,
        field: &'e Positioned<Field>,
    ) -> Result<QueryResponse> {
        match field.node.name.node.as_str() {
            "__type" => Ok(QueryResponse::Json(
                query_context.resolve_type(&field.node).await?,
            )),
            "__schema" => Ok(QueryResponse::Json(
                query_context
                    .executor
                    .schema
                    .resolve_value(query_context, &field.node.selection_set)
                    .await?,
            )),
            "__typename" => {
                let typename = match self.ty {
                    OperationType::Query => QUERY_ROOT_TYPENAME,
                    OperationType::Mutation => MUTATION_ROOT_TYPENAME,
                    OperationType::Subscription => SUBSCRIPTION_ROOT_TYPENAME,
                };
                Ok(QueryResponse::Json(JsonValue::String(typename.to_string())))
            }
            _ => {
                query_context
                    .executor
                    .system
                    .resolve(field, &self.ty, query_context)
                    .await
            }
        }
    }
}
