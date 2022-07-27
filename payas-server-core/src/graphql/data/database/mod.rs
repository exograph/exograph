pub use database_execution_error::DatabaseExecutionError;
pub use database_mutation::DatabaseMutation;
pub use database_query::DatabaseQuery;
pub use database_system_context::DatabaseSystemContext;

mod cast;
mod create_data_param_mapper;
mod database_execution_error;
mod database_mutation;
mod database_query;
mod database_system_context;
mod limit_offset_mapper;
mod order_by_mapper;
mod predicate_mapper;
mod sql_mapper;
mod update_data_param_mapper;

use std::collections::HashMap;

use async_graphql_value::ConstValue;
use postgres_types::FromSqlOwned;
use tokio_postgres::Row;

use payas_sql::{AbstractPredicate, ColumnPath, ColumnPathLink, PhysicalColumn, PhysicalTable};

use crate::graphql::request_context::RequestContext;

use predicate_mapper::PredicateParameterMapper;

use payas_model::model::{
    column_id::ColumnId,
    operation::{OperationReturnType, Query},
    predicate::{ColumnIdPath, ColumnIdPathLink, PredicateParameter},
    system::ModelSystem,
    GqlCompositeType, GqlTypeKind,
};

use self::sql_mapper::SQLOperationKind;

use crate::graphql::data::access_solver;

pub type Arguments = HashMap<String, ConstValue>;

pub async fn compute_sql_access_predicate<'a>(
    return_type: &OperationReturnType,
    kind: &SQLOperationKind,
    system_context: &DatabaseSystemContext<'a>,
    request_context: &'a RequestContext<'a>,
) -> AbstractPredicate<'a> {
    let return_type = return_type.typ(system_context.system);

    match &return_type.kind {
        GqlTypeKind::Primitive => AbstractPredicate::True,
        GqlTypeKind::Composite(GqlCompositeType { access, .. }) => {
            let access_expr = match kind {
                SQLOperationKind::Create => &access.creation,
                SQLOperationKind::Retrieve => &access.read,
                SQLOperationKind::Update => &access.update,
                SQLOperationKind::Delete => &access.delete,
            };
            access_solver::solve_access(access_expr, request_context, system_context.system).await
        }
    }
}

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
    system_context: &DatabaseSystemContext<'a>,
) -> Result<AbstractPredicate<'a>, DatabaseExecutionError> {
    let mapped = predicate_param
        .as_ref()
        .and_then(|predicate_parameter| {
            let argument_value = find_arg(arguments, &predicate_parameter.name);
            argument_value.map(|argument_value| {
                predicate_parameter.map_to_predicate(argument_value, None, system_context)
            })
        })
        .transpose()?;

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

pub fn get_argument_field<'a>(
    argument_value: &'a ConstValue,
    field_name: &str,
) -> Option<&'a ConstValue> {
    match argument_value {
        ConstValue::Object(value) => value.get(field_name),
        _ => None,
    }
}

pub fn extractor<T: FromSqlOwned>(row: Row) -> Result<T, DatabaseExecutionError> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => Err(DatabaseExecutionError::EmptyRow(err)),
    }
}

///
/// # Returns
/// - A (table associated with the return type, pk query, collection query) tuple.
pub fn return_type_info<'a>(
    return_type: &'a OperationReturnType,
    system_context: &DatabaseSystemContext<'a>,
) -> (&'a PhysicalTable, &'a Query, &'a Query) {
    let system = &system_context.system;
    let typ = return_type.typ(system);

    match &typ.kind {
        GqlTypeKind::Primitive => panic!(""),
        GqlTypeKind::Composite(kind) => (
            &system.tables[kind.get_table_id()],
            &system.queries[kind.get_pk_query()],
            &system.queries[kind.get_collection_query()],
        ),
    }
}
