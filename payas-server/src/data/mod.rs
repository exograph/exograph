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

use predicate_mapper::PredicateParameterMapper;

use anyhow::*;
use async_graphql_parser::Positioned;
use async_graphql_value::{ConstValue, Name};

use crate::{execution::query_context::QueryContext, sql::predicate::Predicate};

use payas_model::{
    model::{predicate::PredicateParameter, system::ModelSystem, GqlType, GqlTypeKind},
    sql::{column::Column, TableQuery},
};

use self::predicate_mapper::TableJoin;

pub type Arguments = [(Positioned<Name>, Positioned<ConstValue>)];

fn find_arg<'a>(arguments: &'a Arguments, arg_name: &str) -> Option<&'a ConstValue> {
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
    additional_predicate: Predicate<'a>,
    query_context: &'a QueryContext<'a>,
) -> Result<(Predicate<'a>, Option<TableJoin<'a>>)> {
    let mapped = predicate_param
        .as_ref()
        .and_then(|predicate_parameter| {
            let argument_value = find_arg(arguments, &predicate_parameter.name);
            argument_value.map(|argument_value| {
                predicate_parameter.map_to_predicate(argument_value, query_context)
            })
        })
        .transpose()
        .context("While mapping predicate parameters to SQL")?;

    let res = match mapped {
        Some((predicate, join)) => (Predicate::and(predicate, additional_predicate), join),
        None => (additional_predicate, None),
    };

    Ok(res)
}

pub fn compute_join<'a>(join_info: TableJoin<'a>, system: &'a ModelSystem) -> TableQuery<'a> {
    join_info.dependencies.into_iter().fold(
        TableQuery::Physical(join_info.table),
        |acc, (join_column_dependency, join_table)| {
            let join_predicate = Predicate::Eq(
                Column::Physical(join_column_dependency.self_column_id.get_column(system)).into(),
                Column::Physical(
                    join_column_dependency
                        .dependent_column_id
                        .unwrap()
                        .get_column(system),
                )
                .into(),
            );

            let join_table_query = compute_join(join_table, system);

            acc.join(join_table_query, join_predicate.into())
        },
    )
}

pub fn compute_table_query<'a>(
    join: Option<TableJoin<'a>>,
    root_type: &GqlType,
    system: &'a ModelSystem,
) -> Result<TableQuery<'a>> {
    match join {
        Some(join) => Ok(compute_join(join, system)),
        None => {
            if let GqlTypeKind::Composite(composite_root_type) = &root_type.kind {
                let root_physical_table = &system.tables[composite_root_type.get_table_id()];
                Ok(TableQuery::Physical(root_physical_table))
            } else {
                Err(anyhow!("Expected a composite type"))
            }
        }
    }
}
