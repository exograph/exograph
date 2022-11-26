use async_graphql_value::ConstValue;

use postgres_model::model::ModelPostgresSystem;

use crate::util::{find_arg, Arguments};

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

    fn param_name(&self) -> &str;
}

pub(crate) fn extract_and_map<'a, P, R>(
    param: P,
    arguments: &'a Arguments,
    subsystem: &'a ModelPostgresSystem,
) -> Result<Option<R>, PostgresExecutionError>
where
    P: SQLMapper<'a, R>,
{
    let argument_value = find_arg(arguments, param.param_name());
    argument_value
        .map(|argument_value| param.to_sql(argument_value, subsystem))
        .transpose()
}
