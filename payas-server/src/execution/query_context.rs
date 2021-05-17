use std::collections::HashMap;

use async_graphql_parser::{
    types::{BaseType, Field, FragmentDefinition, FragmentSpread, OperationDefinition, Type},
    Positioned,
};
use async_graphql_value::{Name, Value};
use payas_model::{model::system::ModelSystem, sql::database::Database};
use serde_json::{Map, Value as JsonValue};

use super::resolver::*;

use crate::{data::data_resolver::DataResolver, introspection::schema::Schema};

#[derive(Debug, Clone)]
pub struct QueryContext<'a> {
    pub operation_name: Option<&'a str>,
    pub fragment_definitions: HashMap<Name, Positioned<FragmentDefinition>>,
    pub variables: &'a Option<&'a Map<String, JsonValue>>,
    pub schema: &'a Schema,
    pub system: &'a ModelSystem,
    pub database: &'a Database,
}

#[derive(Debug, Clone)]
pub enum QueryResponse {
    Json(JsonValue),
    Raw(String),
}

impl<'qc> QueryContext<'qc> {
    pub fn resolve_operation<'b>(
        &self,
        operation: (Option<&Name>, &'b Positioned<OperationDefinition>),
    ) -> Vec<(String, QueryResponse)> {
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

    fn resolve_type(&self, field: &Field) -> JsonValue {
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
            JsonValue::Null
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
    ) -> QueryResponse {
        if field.node.name.node == "__type" {
            QueryResponse::Json(query_context.resolve_type(&field.node))
        } else if field.node.name.node == "__schema" {
            QueryResponse::Json(
                query_context
                    .schema
                    .resolve_value(query_context, &field.node.selection_set),
            )
        } else {
            query_context
                .system
                .resolve(&field, &self.ty, query_context)
        }
    }
}
