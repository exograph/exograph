use std::collections::HashMap;

use anyhow::Result;
use async_graphql_parser::{
    types::{
        BaseType, Field, FragmentDefinition, FragmentSpread, OperationDefinition, OperationType,
        Type,
    },
    Positioned,
};
use async_graphql_value::{Name, Value};
use serde_json::{Map, Value as JsonValue};

use super::{executor::Executor, resolver::*};

use crate::{data::data_resolver::DataResolver, introspection::schema::*};

pub struct QueryContext<'a> {
    pub operation_name: Option<&'a str>,
    pub fragment_definitions: HashMap<Name, Positioned<FragmentDefinition>>,
    pub variables: &'a Option<&'a Map<String, JsonValue>>,
    pub executor: &'a Executor<'a>,
    pub request_context: &'a serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum QueryResponse {
    Json(JsonValue),
    Raw(Option<String>),
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
