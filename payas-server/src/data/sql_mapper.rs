use anyhow::Result;

use super::{access_solver, operation_context::OperationContext};
use async_graphql_parser::{types::Field, Positioned};
use async_graphql_value::Value;
use payas_model::{
    model::{
        operation::{Mutation, OperationReturnType},
        GqlCompositeTypeKind, GqlTypeKind,
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
    fn map_to_sql(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<TransactionScript<'a>>;
}

pub enum OperationKind {
    Create,
    Retrieve,
    Update,
    Delete,
}
pub fn compute_access_predicate<'a>(
    return_type: &OperationReturnType,
    kind: &OperationKind,
    operation_context: &'a OperationContext<'a>,
) -> &'a Predicate<'a> {
    let return_type = return_type.typ(operation_context.query_context.system);

    match &return_type.kind {
        GqlTypeKind::Primitive => &Predicate::True,
        GqlTypeKind::Composite(GqlCompositeTypeKind { access, .. }) => {
            let access_expr = match kind {
                OperationKind::Create => &access.creation,
                OperationKind::Retrieve => &access.read,
                OperationKind::Update => &access.update,
                OperationKind::Delete => &access.delete,
            };
            access_solver::reduce_access(
                access_expr,
                operation_context.query_context.request_context,
                operation_context,
            )
        }
    }
}
