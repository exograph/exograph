pub mod cast;
mod create_data_param_mapper;
mod limit_offset_mapper;

mod order_by_mapper;
pub mod predicate_mapper;

pub mod database_query;
pub mod sql_mapper;
mod update_data_param_mapper;

use std::collections::HashMap;

use postgres_types::FromSqlOwned;
use predicate_mapper::PredicateParameterMapper;

use crate::graphql::validation::field::ValidatedField;
use async_graphql_value::ConstValue;
use tokio_postgres::Row;

use sql_mapper::{SQLInsertMapper, SQLUpdateMapper};

use payas_sql::{
    AbstractDelete, AbstractInsert, AbstractPredicate, AbstractSelect, AbstractUpdate, ColumnPath,
    ColumnPathLink, PhysicalColumn, PhysicalTable,
};

use crate::graphql::{
    execution::system_context::SystemContext,
    execution_error::{ExecutionError, WithContext},
    request_context::RequestContext,
};

use payas_model::model::{
    column_id::ColumnId,
    operation::{CreateDataParameter, Mutation, OperationReturnType, Query, UpdateDataParameter},
    predicate::{ColumnIdPath, ColumnIdPathLink, PredicateParameter},
    system::ModelSystem,
    GqlCompositeType, GqlTypeKind,
};

use self::sql_mapper::SQLOperationKind;

use super::{data::access_solver, execution_error::DatabaseExecutionError};

pub type Arguments = HashMap<String, ConstValue>;

pub async fn compute_sql_access_predicate<'a>(
    return_type: &OperationReturnType,
    kind: &SQLOperationKind,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> AbstractPredicate<'a> {
    let return_type = return_type.typ(&system_context.system);

    match &return_type.kind {
        GqlTypeKind::Primitive => AbstractPredicate::True,
        GqlTypeKind::Composite(GqlCompositeType { access, .. }) => {
            let access_expr = match kind {
                SQLOperationKind::Create => &access.creation,
                SQLOperationKind::Retrieve => &access.read,
                SQLOperationKind::Update => &access.update,
                SQLOperationKind::Delete => &access.delete,
            };
            access_solver::solve_access(access_expr, request_context, &system_context.system).await
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
    system_context: &'a SystemContext,
) -> Result<AbstractPredicate<'a>, ExecutionError> {
    let mapped = predicate_param
        .as_ref()
        .and_then(|predicate_parameter| {
            let argument_value = find_arg(arguments, &predicate_parameter.name);
            argument_value.map(|argument_value| {
                predicate_parameter.map_to_predicate(argument_value, None, system_context)
            })
        })
        .transpose()
        .with_context("While mapping predicate parameters to SQL".into())?;

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

pub async fn create_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a CreateDataParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractInsert<'a>, ExecutionError> {
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Create,
        system_context,
        request_context,
    )
    .await;

    // TODO: Allow access_predicate to have a residue that we can evaluate against data_param
    // See issue #69
    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(ExecutionError::Authorization);
    }

    let argument_value = find_arg(&field.arguments, &data_param.name).unwrap();

    data_param.insert_operation(mutation, select, argument_value, system_context)
}

pub async fn delete_operation<'a>(
    mutation: &'a Mutation,
    predicate_param: &'a PredicateParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractDelete<'a>, ExecutionError> {
    let (table, _, _) = return_type_info(mutation, system_context);

    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Delete,
        system_context,
        request_context,
    )
    .await;

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(ExecutionError::Authorization);
    }

    let predicate = compute_predicate(
        Some(predicate_param),
        &field.arguments,
        AbstractPredicate::True,
        system_context,
    )
    .with_context(format!(
        "During predicate computation for parameter {}",
        predicate_param.name
    ))?;

    Ok(AbstractDelete {
        table,
        predicate: Some(predicate),
        selection: select,
    })
}

pub async fn update_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a UpdateDataParameter,
    predicate_param: &'a PredicateParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractUpdate<'a>, ExecutionError> {
    // Access control as well as predicate computation isn't working fully yet. Specifically,
    // nested predicates aren't working.
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Update,
        system_context,
        request_context,
    )
    .await;

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(ExecutionError::Authorization);
    }

    // TODO: https://github.com/payalabs/payas/issues/343
    let predicate = compute_predicate(
        Some(predicate_param),
        &field.arguments,
        AbstractPredicate::True,
        system_context,
    )
    .with_context(format!(
        "During predicate computation for parameter {}",
        predicate_param.name
    ))?;

    let argument_value = find_arg(&field.arguments, &data_param.name);
    argument_value
        .map(|argument_value| {
            data_param.update_operation(mutation, predicate, select, argument_value, system_context)
        })
        .unwrap()
}

///
/// # Returns
/// - A (table associated with the return type, pk query, collection query) tuple.
pub fn return_type_info<'a>(
    mutation: &'a Mutation,
    system_context: &'a SystemContext,
) -> (&'a PhysicalTable, &'a Query, &'a Query) {
    let system = &system_context.system;
    let typ = mutation.return_type.typ(system);

    match &typ.kind {
        GqlTypeKind::Primitive => panic!(""),
        GqlTypeKind::Composite(kind) => (
            &system.tables[kind.get_table_id()],
            &system.queries[kind.get_pk_query()],
            &system.queries[kind.get_collection_query()],
        ),
    }
}
