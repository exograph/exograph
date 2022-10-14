use async_graphql_value::ConstValue;

use core_resolver::system_resolver::SystemResolver;
use payas_sql::{AbstractInsert, AbstractPredicate, AbstractSelect, AbstractUpdate};
use postgres_model::{model::ModelPostgresSystem, operation::OperationReturnType};

use super::postgres_execution_error::PostgresExecutionError;
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
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<R, PostgresExecutionError>;
}

pub trait SQLInsertMapper<'a> {
    fn insert_operation(
        &'a self,
        return_type: OperationReturnType,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        subsystem: &'a ModelPostgresSystem,
        system_resolver: &'a SystemResolver,
    ) -> Result<AbstractInsert, PostgresExecutionError>;
}

pub(crate) trait SQLUpdateMapper<'a> {
    fn update_operation(
        &'a self,
        return_type: &'a OperationReturnType,
        predicate: AbstractPredicate<'a>,
        select: AbstractSelect<'a>,
        argument: &'a ConstValue,
        subsystem: &'a ModelPostgresSystem,
        system_resolver: &'a SystemResolver,
    ) -> Result<AbstractUpdate, PostgresExecutionError>;
}
