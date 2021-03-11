use crate::DataContext;
use graphql_parser::query::{FragmentDefinition, FragmentSpread, SelectionSet};
use serde_json::{Map, Value};

use super::resolver::*;
use graphql_parser::{query::Field, schema::Type};

use crate::introspection::schema::Schema;

#[derive(Debug, Clone)]
pub struct QueryContext<'a> {
    pub operation_name: &'a str,
    pub fragment_definitions: Vec<FragmentDefinition<'a, String>>,
    pub variables: &'a Option<&'a Map<String, Value>>,
    pub schema: &'a Schema<'a>,
    pub data_context: &'a DataContext<'a>,
}

#[derive(Debug, Clone)]
pub enum QueryResponse {
    Json(Value),
    Raw(String)
}

impl<'qc> QueryContext<'qc> {
    pub fn resolve<'b>(
        &self,
        selection_set: &'b SelectionSet<'_, String>,
    ) -> Vec<(String, QueryResponse)> {
        self.resolve_selection_set(self, selection_set)
    }

    pub fn fragment_definition(&self, fragment: &FragmentSpread<String>) -> Option<&FragmentDefinition<'qc, String>> {
        self
            .fragment_definitions
            .iter()
            .find(|fd| fd.name == fragment.fragment_name)
    }

    fn resolve_type(&self, field: &Field<'_, String>) -> Value {
        let name_arg = field.arguments.iter().find(|arg| arg.0 == "name").unwrap();

        if let graphql_parser::query::Value::String(name_specified) = &name_arg.1 {
            let tpe: Type<String> = Type::NamedType(name_specified.to_owned());
            tpe.resolve_value(self, &field.selection_set)
        } else {
            Value::Null
        }
    }
}


/**
 Go through the FieldResolver (thus through the generic support offered by Resolver) and
 so that we can support fragments in top-level queries such as:
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
 */
impl<'b> FieldResolver<QueryResponse> for QueryContext<'b> {
    fn resolve_field<'a>(
        &'a self,
        _query_context: &QueryContext<'_>,
        field: &Field<'_, String>,
    ) -> QueryResponse {
        if field.name == "__type" {
            QueryResponse::Json(self.resolve_type(&field))
        } else if field.name == "__schema" {
            QueryResponse::Json(self.schema.resolve_value(self, &field.selection_set))
        } else {
            self.data_context.resolve(&field, self)
        }
    }
}