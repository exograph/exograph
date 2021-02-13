use super::{query_context, schema::Schema};
use graphql_parser::{
    parse_query,
    query::{Definition, FragmentDefinition, OperationDefinition, SelectionSet},
};
use query_context::*;
use serde_json::{json, Map, Value};

pub fn execute<'a>(
    schema: &'a Schema<'a>,
    operation_name: &'a str,
    query_str: &'a str,
    variables: &'a Option<&'a Map<String, Value>>,
) -> String {
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

    let query_context = QueryContext {
        operation_name,
        fragment_definitions,
        variables,
        schema: schema,
    };

    // TODO: Acertain that only one of query, mutation, and subscription is present
    let parts: Map<String, Value> = queries
        .iter()
        .flat_map(|query| query_context.resolve(query))
        .collect();

    let response = Value::Object(parts);

    json!({ "data": response }).to_string()
}
