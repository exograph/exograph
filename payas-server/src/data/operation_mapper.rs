use anyhow::Result;

use super::{access_solver, operation_context::OperationContext};
use async_graphql_parser::{types::Field, Positioned};
use async_graphql_value::Value;
use payas_model::{
    model::{
        mapped_arena::SerializableSlabIndex,
        operation::{Mutation, OperationReturnType},
        service::{ServiceMethod, ServiceMethodType},
        GqlCompositeType, GqlCompositeTypeKind, GqlTypeKind,
    },
    sql::{predicate::Predicate, transaction::TransactionScript, Select},
};

pub trait SQLMapper<'a, R> {
    fn map_to_sql(
        &'a self,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<R>;
}

pub trait SQLUpdateMapper<'a> {
    fn update_script(
        &'a self,
        mutation: &'a Mutation,
        predicate: &'a Predicate,
        select: Select<'a>,
        argument: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<TransactionScript>;
}
pub trait OperationResolver<'a> {
    fn resolve_operation(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<OperationResolverResult<'a>>;
}

pub enum OperationResolverResult<'a> {
    SQLOperation(TransactionScript<'a>),
    DenoOperation(SerializableSlabIndex<ServiceMethod>),
}

pub enum SQLOperationKind {
    Create,
    Retrieve,
    Update,
    Delete,
}

pub fn compute_sql_access_predicate<'a>(
    return_type: &OperationReturnType,
    kind: &SQLOperationKind,
    operation_context: &'a OperationContext<'a>,
) -> &'a Predicate<'a> {
    let return_type = return_type.typ(operation_context.get_system());

    match &return_type.kind {
        GqlTypeKind::Primitive => &Predicate::True,
        GqlTypeKind::Composite(GqlCompositeType { access, .. }) => {
            let access_expr = match kind {
                SQLOperationKind::Create => &access.creation,
                SQLOperationKind::Retrieve => &access.read,
                SQLOperationKind::Update => &access.update,
                SQLOperationKind::Delete => &access.delete,
            };
            access_solver::reduce_access(
                access_expr,
                operation_context.query_context.request_context,
                operation_context,
            )
        }
    }
}

pub fn compute_service_access_predicate<'a>(
    return_type: &OperationReturnType,
    method: &'a ServiceMethod,
    operation_context: &'a OperationContext<'a>,
) -> &'a Predicate<'a> {
    let return_type = return_type.typ(operation_context.get_system());

    let type_level_access = match &return_type.kind {
        GqlTypeKind::Primitive => &Predicate::True,
        GqlTypeKind::Composite(GqlCompositeType {
            access,
            kind: GqlCompositeTypeKind::NonPersistent,
            ..
        }) => {
            let access_expr = match &method.operation_kind {
                ServiceMethodType::Query(_) => &access.read, // query
                ServiceMethodType::Mutation(_) => &access.creation, // mutation
            };
            access_solver::reduce_access(
                access_expr,
                operation_context.query_context.request_context,
                operation_context,
            )
        }
        _ => panic!(),
    };

    let method_access_expr = match &method.operation_kind {
        ServiceMethodType::Query(_) => &method.access.read, // query
        ServiceMethodType::Mutation(_) => &method.access.creation, // mutation
    };

    let method_level_access = access_solver::reduce_access(
        method_access_expr,
        operation_context.query_context.request_context,
        operation_context,
    );

    if matches!(type_level_access, &Predicate::False)
        || matches!(method_level_access, &Predicate::False)
    {
        &Predicate::False // deny if either access check fails
    } else {
        &Predicate::True
    }
}
