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

use anyhow::*;
use async_graphql_parser::Positioned;
use async_graphql_value::{Name, Value};
use maybe_owned::MaybeOwned;

use crate::{execution::query_context::QueryContext, sql::predicate::Predicate};

use payas_model::model::predicate::PredicateParameter;

use self::operation_mapper::SQLMapper;

type Arguments = [(Positioned<Name>, Positioned<Value>)];

fn find_arg<'a>(arguments: &'a Arguments, arg_name: &str) -> Option<&'a Value> {
    arguments.iter().find_map(|argument| {
        let (argument_name, argument_value) = argument;
        if arg_name == argument_name.node {
            Some(&argument_value.node)
        } else {
            None
        }
    })
}

fn compute_predicate<'a>(
    predicate_param: Option<&'a PredicateParameter>,
    arguments: &'a Arguments,
    additional_predicate: MaybeOwned<'a, Predicate<'a>>,
    query_context: &'a QueryContext<'a>,
) -> Result<MaybeOwned<'a, Predicate<'a>>> {
    let predicate = predicate_param
        .as_ref()
        .and_then(|predicate_parameter| {
            let argument_value = find_arg(arguments, &predicate_parameter.name);
            argument_value
                .map(|argument_value| predicate_parameter.map_to_sql(argument_value, query_context))
        })
        .transpose()
        .context("While mapping predicate parameters to SQL")?;

    let predicate = match predicate {
        Some(predicate) => {
            Predicate::And(Box::new(predicate.into()), Box::new(additional_predicate)).into()
        }
        None => additional_predicate,
    };

    Ok(predicate)
}
