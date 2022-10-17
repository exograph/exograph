use async_graphql_value::{indexmap::IndexMap, ConstValue};

use crate::{access_solver::PostgresAccessSolver, sql_mapper::SQLOperationKind};
use core_resolver::access_solver::AccessSolver;
use core_resolver::request_context::RequestContext;
use payas_sql::{AbstractPredicate, PhysicalTable};
use postgres_model::{
    column_path::{ColumnIdPath, ColumnIdPathLink},
    model::ModelPostgresSystem,
    operation::{OperationReturnType, PostgresQuery},
    types::{PostgresCompositeType, PostgresTypeKind},
};

pub type Arguments = IndexMap<String, ConstValue>;

pub(crate) async fn compute_sql_access_predicate<'a>(
    return_type: &OperationReturnType,
    kind: &SQLOperationKind,
    subsystem: &'a ModelPostgresSystem,
    request_context: &'a RequestContext<'a>,
) -> AbstractPredicate<'a> {
    let return_type = return_type.typ(subsystem);
    let access_solver = PostgresAccessSolver::new(request_context, subsystem);

    match &return_type.kind {
        PostgresTypeKind::Primitive => AbstractPredicate::True,
        PostgresTypeKind::Composite(PostgresCompositeType { access, .. }) => {
            let access_expr = match kind {
                SQLOperationKind::Create => &access.creation,
                SQLOperationKind::Retrieve => &access.read,
                SQLOperationKind::Update => &access.update,
                SQLOperationKind::Delete => &access.delete,
            };
            access_solver.solve(access_expr).await.0
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
    subsystem: &'a ModelPostgresSystem,
) -> (&'a PhysicalTable, &'a PostgresQuery, &'a PostgresQuery) {
    let typ = return_type.typ(subsystem);

    match &typ.kind {
        PostgresTypeKind::Primitive => panic!(""),
        PostgresTypeKind::Composite(kind) => (
            &subsystem.tables[kind.table_id],
            &subsystem.queries[kind.pk_query],
            &subsystem.queries[kind.collection_query],
        ),
    }
}
