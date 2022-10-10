use async_graphql_value::{indexmap::IndexMap, ConstValue};

use crate::{access_solver, sql_mapper::SQLOperationKind};
use payas_core_resolver::{request_context::RequestContext, system_resolver::SystemResolver};
use payas_database_model::{
    column_path::{ColumnIdPath, ColumnIdPathLink},
    model::ModelDatabaseSystem,
    operation::{DatabaseQuery, OperationReturnType},
    types::{DatabaseCompositeType, DatabaseTypeKind},
};
use payas_sql::{AbstractPredicate, PhysicalTable};

pub type Arguments = IndexMap<String, ConstValue>;

pub(crate) async fn compute_sql_access_predicate<'a>(
    return_type: &OperationReturnType,
    kind: &SQLOperationKind,
    subsystem: &'a ModelDatabaseSystem,
    system_resolver: &'a SystemResolver,
    request_context: &'a RequestContext<'a>,
) -> AbstractPredicate<'a> {
    let return_type = return_type.typ(subsystem);

    match &return_type.kind {
        DatabaseTypeKind::Primitive => AbstractPredicate::True,
        DatabaseTypeKind::Composite(DatabaseCompositeType { access, .. }) => {
            let access_expr = match kind {
                SQLOperationKind::Create => &access.creation,
                SQLOperationKind::Retrieve => &access.read,
                SQLOperationKind::Update => &access.update,
                SQLOperationKind::Delete => &access.delete,
            };
            access_solver::solve_access(access_expr, request_context, subsystem, &system_resolver)
                .await
        }
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
    return_type: &'a OperationReturnType,
    subsystem: &'a ModelDatabaseSystem,
) -> (&'a PhysicalTable, &'a DatabaseQuery, &'a DatabaseQuery) {
    let typ = return_type.typ(subsystem);

    match &typ.kind {
        DatabaseTypeKind::Primitive => panic!(""),
        DatabaseTypeKind::Composite(kind) => (
            &subsystem.tables[kind.table_id],
            &subsystem.queries[kind.pk_query],
            &subsystem.queries[kind.collection_query],
        ),
    }
}
