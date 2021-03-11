use super::query_context;
use crate::introspection::schema::Schema;
use crate::DataContext;
use graphql_parser::{
    parse_query,
    query::{Definition, FragmentDefinition, OperationDefinition, SelectionSet},
};
use query_context::*;
use serde_json::{Map, Value};

pub fn create_query_context<'a>(
    data_context: &'a DataContext,
    schema: &'a Schema<'a>,
    operation_name: &'a str,
    query_str: &'a str,
    variables: &'a Option<&'a Map<String, Value>>,
) -> (
    Vec<SelectionSet<'a, String>>,
    Vec<SelectionSet<'a, String>>,
    Vec<SelectionSet<'a, String>>,
    QueryContext<'a>,
) {
    let document = parse_query::<String>(query_str).unwrap();

    let mut queries: Vec<SelectionSet<String>> = vec![];
    let mut mutations: Vec<SelectionSet<String>> = vec![];
    let mut subscriptions: Vec<SelectionSet<String>> = vec![];
    let mut fragment_definitions: Vec<FragmentDefinition<String>> = vec![];

    document.definitions.into_iter().for_each(|definition| {
        match definition {
            Definition::Operation(operation_definition) => {
                match operation_definition {
                    // plain un-named query `{ concerts { ...} venues { ... } }`
                    OperationDefinition::SelectionSet(selection_set) => {
                        queries.push(selection_set);
                    }
                    // named query `query allStuff($x: String) { concerts { ... } venues { ... } }`
                    OperationDefinition::Query(query) => {
                        queries.push(query.selection_set);
                    }
                    OperationDefinition::Mutation(mutation) => {
                        mutations.push(mutation.selection_set)
                    }
                    OperationDefinition::Subscription(subscription) => {
                        subscriptions.push(subscription.selection_set)
                    }
                };
            }
            Definition::Fragment(fragment_definition) => {
                fragment_definitions.push(fragment_definition);
            }
        }
    });

    (
        queries,
        mutations,
        subscriptions,
        QueryContext {
            operation_name,
            fragment_definitions,
            variables,
            schema: &schema,
            data_context,
        },
    )
}

pub fn execute<'a>(
    data_system: &'a DataContext,
    schema: &'a Schema<'a>,
    operation_name: &'a str,
    query_str: &'a str,
    variables: &'a Option<&'a Map<String, Value>>,
) -> String {
    let (queries, _mutations, _subscriptions, query_context) =
        create_query_context(data_system, schema, operation_name, query_str, variables);

    // TODO: Acertain that only one of query, mutation, and subscription is present
    let parts: Vec<(String, QueryResponse)> = queries
        .iter()
        .flat_map(|query| query_context.resolve(query))
        .collect();

    // TODO: More efficient (and ideally zero-copy) way to push the values to network
    let mut response = String::from("{\"data\": {");
    parts.iter().enumerate().for_each(|(index, part)| {
        response.push_str("\"");
        response.push_str(part.0.as_str());
        response.push_str("\":");
        match &part.1 {
            QueryResponse::Json(value) => response.push_str(value.to_string().as_str()),
            QueryResponse::Raw(value) => response.push_str(value.as_str()),
        };
        if index != parts.len() - 1  {
            response.push_str(", ");
        }
    });
    response.push_str("}}");

    response
}
