use async_graphql_value::ConstValue;
use payas_model::model::operation::OperationReturnType;
use payas_sql::{AbstractInsert, AbstractPredicate, AbstractSelect, AbstractUpdate};

use crate::{graphql::execution_error::ExecutionError, SystemContext};
pub enum SQLOperationKind {
    Create,
    Retrieve,
    Update,
    Delete,
}

pub(crate) trait SQLMapper<'a, R> {
    fn map_to_sql(
        &'a self,
        argument: &'a ConstValue,
        system_context: &'a SystemContext,
    ) -> Result<R, ExecutionError>;
}

pub trait SQLInsertMapper<'a> {
    fn insert_operation(
        &'a self,
        return_type: OperationReturnType,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        system_context: &'a SystemContext,
    ) -> Result<AbstractInsert, ExecutionError>;
}

pub trait SQLUpdateMapper<'a> {
    fn update_operation(
        &'a self,
        return_type: &'a OperationReturnType,
        predicate: AbstractPredicate<'a>,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        system_context: &'a SystemContext,
    ) -> Result<AbstractUpdate, ExecutionError>;
}
