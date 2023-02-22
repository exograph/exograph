use async_graphql_value::{indexmap::IndexMap, ConstValue};
use postgres_model::types::EntityType;

use crate::{
    access_solver::PostgresAccessSolver, postgres_execution_error::PostgresExecutionError,
    sql_mapper::SQLOperationKind,
};
use core_plugin_interface::core_model::types::OperationReturnType;
use core_plugin_interface::core_resolver::{
    access_solver::AccessSolver, request_context::RequestContext,
};
use payas_sql::{AbstractPredicate, PhysicalTable};
use postgres_model::{
    column_path::{ColumnIdPath, ColumnIdPathLink},
    query::{CollectionQuery, PkQuery},
    subsystem::PostgresSubsystem,
};

pub type Arguments = IndexMap<String, ConstValue>;

// TODO: Allow access_predicate to have a residue that we can evaluate against data_param
// See issue #69
pub(crate) async fn check_access<'a>(
    return_type: &'a OperationReturnType<EntityType>,
    kind: &SQLOperationKind,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate<'a>, PostgresExecutionError> {
    let return_type = return_type.typ(&subsystem.entity_types);
    let access_solver = PostgresAccessSolver::new(request_context, subsystem);

    let access_predicate = {
        let access_expr = match kind {
            SQLOperationKind::Create => &return_type.access.creation,
            SQLOperationKind::Retrieve => &return_type.access.read,
            SQLOperationKind::Update => &return_type.access.update,
            SQLOperationKind::Delete => &return_type.access.delete,
        };
        access_solver.solve(access_expr).await.0
    };

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        Err(PostgresExecutionError::Authorization)
    } else {
        Ok(access_predicate)
    }
}

pub fn find_arg<'a>(arguments: &'a Arguments, arg_name: &str) -> Option<&'a ConstValue> {
    arguments.iter().find_map(|argument| {
        let (argument_name, argument_value) = argument;
        if arg_name == argument_name {
            Some(argument_value)
        } else {
            None
        }
    })
}

pub(crate) fn to_column_id_path(
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

pub(crate) fn get_argument_field<'a>(
    argument_value: &'a ConstValue,
    field_name: &str,
) -> Option<&'a ConstValue> {
    match argument_value {
        ConstValue::Object(value) => value.get(field_name),
        _ => None,
    }
}

///
/// # Returns
/// - A (table associated with the return type, pk query, collection query) tuple.
pub(crate) fn return_type_info<'a>(
    return_type: &'a OperationReturnType<EntityType>,
    subsystem: &'a PostgresSubsystem,
) -> (&'a PhysicalTable, &'a PkQuery, &'a CollectionQuery) {
    let typ = return_type.typ(&subsystem.entity_types);

    (
        &subsystem.tables[typ.table_id],
        &subsystem.pk_queries[typ.pk_query],
        &subsystem.collection_queries[typ.collection_query],
    )
}
