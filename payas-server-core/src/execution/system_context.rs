use std::str::FromStr;

use crate::request_context::RequestContext;
use crate::validation::operation::ValidatedOperation;
use crate::OperationsPayload;
use crate::{
    error::ExecutionError, introspection::schema::Schema,
    validation::document_validator::DocumentValidator,
};
use async_graphql_parser::types::ExecutableDocument;
use async_graphql_parser::Pos;

use anyhow::Result;

use crate::deno_integration::ClayDenoExecutorPool;
use payas_model::model::system::ModelSystem;
use payas_sql::DatabaseExecutor;
use tracing::{error, instrument};

use anyhow::{bail, Context};
use async_graphql_parser::types::{BaseType, OperationType, Type};
use async_graphql_value::{ConstValue, Name, Number};
use async_trait::async_trait;
use chrono::prelude::*;
use chrono::DateTime;
use maybe_owned::MaybeOwned;
use payas_sql::{
    array_util::{self, ArrayEntry},
    Column, FloatBits, IntBits, PhysicalColumn, PhysicalColumnType, SQLBytes, SQLParam,
};
use pg_bigdecimal::{BigDecimal, PgNumeric};
use serde_json::Value as JsonValue;

use super::resolver::{FieldResolver, Resolver};

use crate::{
    data::data_resolver::DataResolver,
    introspection::schema::{
        MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME, SUBSCRIPTION_ROOT_TYPENAME,
    },
    validation::field::ValidatedField,
};

const NAIVE_DATE_FORMAT: &str = "%Y-%m-%d";
const NAIVE_TIME_FORMAT: &str = "%H:%M:%S%.f";

/// Encapsulates the information required by the [crate::resolve] function.
///
/// A server implementation should call [crate::create_system_context] and
/// store the returned value, passing a reference to it each time it calls
/// `resolve`.
///
/// For example, in actix, this should be added to the server using `app_data`.
pub struct SystemContext {
    pub(crate) database_executor: DatabaseExecutor,
    pub(crate) deno_execution_pool: ClayDenoExecutorPool,
    pub(crate) system: ModelSystem,
    pub(crate) schema: Schema,
    pub allow_introspection: bool,
}

#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub body: QueryResponseBody,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum QueryResponseBody {
    Json(JsonValue),
    Raw(Option<String>),
}

impl QueryResponseBody {
    pub fn to_json(&self) -> Result<JsonValue> {
        match &self {
            QueryResponseBody::Json(val) => Ok(val.clone()),
            QueryResponseBody::Raw(raw) => {
                if let Some(raw) = raw {
                    Ok(serde_json::from_str(raw)?)
                } else {
                    Ok(JsonValue::Null)
                }
            }
        }
    }
}

impl SystemContext {
    pub async fn execute<'e>(
        &'e self,
        operations_payload: OperationsPayload,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Vec<(String, QueryResponse)>> {
        self.execute_with_request_context(operations_payload, request_context)
            .await
    }

    // A version of execute that is suitable to be exposed through a shim to services
    #[instrument(
        name = "OperationsExecutor::execute_with_request_context"
        skip_all
        )]
    pub async fn execute_with_request_context(
        &self,
        operations_payload: OperationsPayload,
        request_context: &RequestContext<'_>,
    ) -> Result<Vec<(String, QueryResponse)>> {
        let operation = self.validate_operation(operations_payload)?;

        self.resolve_operation(operation, request_context).await
    }

    #[instrument(skip(self, operations_payload))]
    fn validate_operation<'e>(
        &'e self,
        operations_payload: OperationsPayload,
    ) -> Result<ValidatedOperation, ExecutionError> {
        let document = parse_query(operations_payload.query)?;

        let document_validator = DocumentValidator::new(
            &self.schema,
            operations_payload.operation_name,
            operations_payload.variables,
        );

        document_validator.validate(document)
    }

    #[instrument(
        name = "OperationsContext::resolve_operation"
        skip_all
        fields(
            operation.name,
            %operation.typ
            )
        )]
    pub async fn resolve_operation<'b>(
        &self,
        operation: ValidatedOperation,
        request_context: &'b RequestContext<'b>,
    ) -> Result<Vec<(String, QueryResponse)>> {
        operation
            .resolve_fields(&operation.fields, self, request_context)
            .await
    }

    async fn resolve_type<'b>(
        &self,
        field: &ValidatedField,
        request_context: &'b RequestContext<'b>,
    ) -> Result<JsonValue> {
        let type_name = &field
            .arguments
            .iter()
            .find(|arg| arg.0 == "name")
            .unwrap()
            .1;

        if let ConstValue::String(name_specified) = &type_name {
            let tpe: Type = Type {
                base: BaseType::Named(Name::new(name_specified)),
                nullable: true,
            };
            tpe.resolve_value(&field.subfields, self, request_context)
                .await
        } else {
            Ok(JsonValue::Null)
        }
    }
}

#[instrument(name = "system_context::parse_query")]
fn parse_query(query: String) -> Result<ExecutableDocument, ExecutionError> {
    async_graphql_parser::parse_query(query).map_err(|error| {
        error!(%error, "Failed to parse query");
        let (message, pos1, pos2) = match error {
            async_graphql_parser::Error::Syntax {
                message,
                start,
                end,
            } => (format!("Syntax error {message}"), start, end),
            async_graphql_parser::Error::MultipleRoots { root, schema, pos } => {
                (format!("Multiple roots of {root} type"), schema, Some(pos))
            }
            async_graphql_parser::Error::MissingQueryRoot { pos } => {
                ("Missing query root".to_string(), pos, None)
            }
            async_graphql_parser::Error::MultipleOperations {
                anonymous,
                operation,
            } => (
                "Multiple operations".to_string(),
                anonymous,
                Some(operation),
            ),
            async_graphql_parser::Error::OperationDuplicated {
                operation: _,
                first,
                second,
            } => ("Operation duplicated".to_string(), first, Some(second)),
            async_graphql_parser::Error::FragmentDuplicated {
                fragment,
                first,
                second,
            } => (
                format!("Fragment {fragment} duplicated"),
                first,
                Some(second),
            ),
            async_graphql_parser::Error::MissingOperation => {
                ("Missing operation".to_string(), Pos::default(), None)
            }
            _ => ("Unknown error".to_string(), Pos::default(), None),
        };

        ExecutionError::QueryParsingFailed(message, pos1, pos2)
    })
}

pub fn literal_column<'a>(
    value: &ConstValue,
    associated_column: &PhysicalColumn,
) -> Result<Column<'a>> {
    cast_value(value, &associated_column.typ).map(|value| {
        value
            .map(|v| Column::Literal(MaybeOwned::Owned(v)))
            .unwrap_or(Column::Null)
    })
}

pub fn get_argument_field<'a>(
    argument_value: &'a ConstValue,
    field_name: &str,
) -> Option<&'a ConstValue> {
    match argument_value {
        ConstValue::Object(value) => value.get(field_name),
        _ => None,
    }
}

pub fn cast_value(
    value: &ConstValue,
    destination_type: &PhysicalColumnType,
) -> Result<Option<Box<dyn SQLParam>>> {
    match value {
        ConstValue::Number(number) => cast_number(number, destination_type).map(Some),
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

fn cast_number(
    number: &Number,
    destination_type: &PhysicalColumnType,
) -> Result<Box<dyn SQLParam>> {
    let result: Box<dyn SQLParam> = match destination_type {
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
            bail!("Number literals cannot be specified for decimal fields")
        }
        PhysicalColumnType::ColumnReference { ref_pk_type, .. } => {
            // TODO assumes that `id` columns are always integers
            cast_number(number, ref_pk_type)?
        }
        // TODO: Expand for other number types such as float
        _ => bail!("Unexpected destination_type for number value"),
    };

    Ok(result)
}

fn cast_string(string: &str, destination_type: &PhysicalColumnType) -> Result<Box<dyn SQLParam>> {
    let value: Box<dyn SQLParam> = match destination_type {
        PhysicalColumnType::Numeric { .. } => {
            let decimal =
                match string {
                    "NaN" => PgNumeric { n: None },
                    _ => PgNumeric {
                        n: Some(BigDecimal::from_str(string).with_context(|| {
                            format!("Could not parse {} into a decimal", string)
                        })?),
                    },
                };

            Box::new(decimal)
        }

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

        PhysicalColumnType::Uuid => {
            let uuid = uuid::Uuid::parse_str(string)?;
            Box::new(uuid)
        }

        PhysicalColumnType::Array { typ } => cast_string(string, typ)?,

        PhysicalColumnType::ColumnReference { ref_pk_type, .. } => {
            cast_string(string, ref_pk_type)?
        }

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
#[async_trait]
impl FieldResolver<QueryResponse> for ValidatedOperation {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse> {
        let name = field.name.as_str();

        if name.starts_with("__") {
            let body: Result<QueryResponseBody> = if system_context.allow_introspection {
                match name {
                    "__type" => Ok(QueryResponseBody::Json(
                        system_context.resolve_type(field, request_context).await?,
                    )),
                    "__schema" => Ok(QueryResponseBody::Json(
                        system_context
                            .schema
                            .resolve_value(&field.subfields, system_context, request_context)
                            .await?,
                    )),
                    "__typename" => {
                        let typename = match self.typ {
                            OperationType::Query => QUERY_ROOT_TYPENAME,
                            OperationType::Mutation => MUTATION_ROOT_TYPENAME,
                            OperationType::Subscription => SUBSCRIPTION_ROOT_TYPENAME,
                        };
                        Ok(QueryResponseBody::Json(JsonValue::String(
                            typename.to_string(),
                        )))
                    }
                    _ => bail!("No such introspection field {}", name),
                }
            } else {
                bail!("Introspection is not allowed");
            };

            Ok(QueryResponse {
                body: body?,
                headers: vec![],
            })
        } else {
            system_context
                .system
                .resolve(field, &self.typ, system_context, request_context)
                .await
        }
    }
}
