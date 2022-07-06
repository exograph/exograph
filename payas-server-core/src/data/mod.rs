pub mod access_solver;
mod create_data_param_mapper;
pub mod data_resolver;
mod interception;
pub mod limit_offset_mapper;
pub mod mutation_resolver;
pub mod operation_mapper;
pub mod order_by_mapper;
pub mod predicate_mapper;
pub mod query_resolver;
mod update_data_param_mapper;

use std::collections::HashMap;

use payas_sql::{AbstractPredicate, ColumnPath, ColumnPathLink, PhysicalColumn, PhysicalTable};
use predicate_mapper::PredicateParameterMapper;

use anyhow::{Context, Result};
use async_graphql_value::ConstValue;

use crate::execution::operations_context::OperationsContext;

use payas_model::model::{
    column_id::ColumnId,
    predicate::{ColumnIdPath, ColumnIdPathLink, PredicateParameter},
    system::ModelSystem,
};

pub type Arguments = HashMap<String, ConstValue>;

fn find_arg<'a>(arguments: &'a Arguments, arg_name: &str) -> Option<&'a ConstValue> {
    arguments.iter().find_map(|argument| {
        let (argument_name, argument_value) = argument;
        if arg_name == argument_name {
            Some(argument_value)
        } else {
            None
        }
    })
}

fn compute_predicate<'a>(
    predicate_param: Option<&'a PredicateParameter>,
    arguments: &'a Arguments,
    additional_predicate: AbstractPredicate<'a>,
    operations_context: &'a OperationsContext,
) -> Result<AbstractPredicate<'a>> {
    let mapped = predicate_param
        .as_ref()
        .and_then(|predicate_parameter| {
            let argument_value = find_arg(arguments, &predicate_parameter.name);
            argument_value.map(|argument_value| {
                predicate_parameter.map_to_predicate(argument_value, None, operations_context)
            })
        })
        .transpose()
        .context("While mapping predicate parameters to SQL")?;

    let res = match mapped {
        Some(predicate) => {
            AbstractPredicate::And(Box::new(predicate), Box::new(additional_predicate))
        }
        None => additional_predicate,
    };

    Ok(res)
}

pub fn to_column_id_path(
    parent_column_id_path: &Option<ColumnIdPath>,
    next_column_id_path_link: &Option<ColumnIdPathLink>,
) -> Option<ColumnIdPath> {
    match (parent_column_id_path, next_column_id_path_link) {
        (Some(parent_column_id_path), Some(next_column_id_path_link)) => {
            let mut path: Vec<_> = parent_column_id_path.path.clone();
            path.push(next_column_id_path_link.clone());
            Some(ColumnIdPath { path })
        }
        (Some(parent_column_id_path), None) => Some(parent_column_id_path.clone()),
        (None, Some(next_column_id_path_link)) => Some(ColumnIdPath {
            path: vec![next_column_id_path_link.clone()],
        }),
        (None, None) => None,
    }
}

fn to_column_table(column_id: ColumnId, system: &ModelSystem) -> (&PhysicalColumn, &PhysicalTable) {
    let column = column_id.get_column(system);
    let table = &system
        .tables
        .iter()
        .find(|(_, table)| table.name == column.table_name)
        .map(|(_, table)| table)
        .unwrap_or_else(|| panic!("Table {} not found", column.table_name));

    (column, table)
}

fn to_column_path_link<'a>(link: &ColumnIdPathLink, system: &'a ModelSystem) -> ColumnPathLink<'a> {
    ColumnPathLink {
        self_column: to_column_table(link.self_column_id, system),
        linked_column: link
            .linked_column_id
            .map(|linked_column_id| to_column_table(linked_column_id, system)),
    }
}

pub fn to_column_path<'a>(
    parent_column_id_path: &Option<ColumnIdPath>,
    next_column_id_path_link: &Option<ColumnIdPathLink>,
    system: &'a ModelSystem,
) -> ColumnPath<'a> {
    let mut path: Vec<_> = match parent_column_id_path {
        Some(parent_column_id_path) => parent_column_id_path
            .path
            .iter()
            .map(|link| to_column_path_link(link, system))
            .collect(),
        None => vec![],
    };

    if let Some(next_column_id_path_link) = next_column_id_path_link {
        path.push(to_column_path_link(next_column_id_path_link, system));
    }

    ColumnPath::Physical(path)
}

macro_rules! claytip_execute_query {
    ($operations_context:ident, $request_context:ident) => {
        Some(
            &move |query_string: String, variables: Option<serde_json::Map<String, Value>>| {
                async move {
                    // execute query
                    let result = $operations_context
                        .execute_with_request_context(
                            OperationsPayload {
                                operation_name: None,
                                query: query_string,
                                variables,
                            },
                            $request_context.clone(),
                        )
                        .await?;

                    // collate result into a single QueryResponse

                    // since query execution results in a Vec<(String, QueryResponse)>, we want to
                    // extract and collect all HTTP headers generated in QueryResponses
                    let headers = result
                        .iter()
                        .flat_map(|(_, response)| response.headers.clone())
                        .collect::<Vec<_>>();

                    // generate the body
                    let body = result
                        .into_iter()
                        .map(|(name, response)| (name, response.body.to_json().unwrap()))
                        .collect::<Map<_, _>>();

                    Ok(QueryResponse {
                        body: QueryResponseBody::Json(serde_json::Value::Object(body)),
                        headers,
                    })
                }
                .boxed()
            },
        )
    };
}

pub(crate) use claytip_execute_query;
