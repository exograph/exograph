use async_graphql_value::ConstValue;

use core_resolver::system_resolver::SystemResolver;
use database_model::{model::ModelDatabaseSystem, operation::OperationReturnType};
use payas_sql::{AbstractInsert, AbstractPredicate, AbstractSelect, AbstractUpdate};

use super::database_execution_error::DatabaseExecutionError;
pub(crate) enum SQLOperationKind {
    Create,
    Retrieve,
    Update,
    Delete,
}

pub(crate) trait SQLMapper<'a, R> {
    fn map_to_sql(
        &'a self,
        argument: &'a ConstValue,
        subsystem: &'a ModelDatabaseSystem,
    ) -> Result<R, DatabaseExecutionError>;
}

pub trait SQLInsertMapper<'a> {
    fn insert_operation(
        &'a self,
        return_type: OperationReturnType,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        subsystem: &'a ModelDatabaseSystem,
        system_resolver: &'a SystemResolver,
    ) -> Result<AbstractInsert, DatabaseExecutionError>;
}

pub(crate) trait SQLUpdateMapper<'a> {
    fn update_operation(
        &'a self,
        return_type: &'a OperationReturnType,
        predicate: AbstractPredicate<'a>,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        subsystem: &'a ModelDatabaseSystem,
        system_resolver: &'a SystemResolver,
    ) -> Result<AbstractUpdate, DatabaseExecutionError>;
}
