use async_graphql_value::ConstValue;

use postgres_model::model::ModelPostgresSystem;

use super::postgres_execution_error::PostgresExecutionError;
pub(crate) enum SQLOperationKind {
    Create,
    Retrieve,
    Update,
    Delete,
}

pub(crate) trait SQLMapper<'a, R> {
    fn to_sql(
        self,
        argument: &'a ConstValue,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<R, PostgresExecutionError>;
}
