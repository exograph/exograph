use async_graphql_value::ConstValue;

use payas_sql::{Limit, Offset};
use postgres_model::{
    limit_offset::{LimitParameter, OffsetParameter},
    subsystem::PostgresSubsystem,
};

use super::{postgres_execution_error::PostgresExecutionError, sql_mapper::SQLMapper};

fn cast_to_i64(argument: &ConstValue) -> Result<i64, PostgresExecutionError> {
    match argument {
        ConstValue::Number(n) => n
            .as_i64()
            .ok_or_else(|| PostgresExecutionError::Generic(format!("Could not cast {n} to i64"))),
        _ => Err(PostgresExecutionError::Generic("Not a number".into())),
    }
}

impl<'a> SQLMapper<'a, Limit> for &LimitParameter {
    fn to_sql(
        self,
        argument: &'a ConstValue,
        _subsystem: &'a PostgresSubsystem,
    ) -> Result<Limit, PostgresExecutionError> {
        cast_to_i64(argument).map(Limit)
    }

    fn param_name(&self) -> &str {
        &self.name
    }
}

impl<'a> SQLMapper<'a, Offset> for &OffsetParameter {
    fn to_sql(
        self,
        argument: &'a ConstValue,
        _subsystem: &'a PostgresSubsystem,
    ) -> Result<Offset, PostgresExecutionError> {
        cast_to_i64(argument).map(Offset)
    }

    fn param_name(&self) -> &str {
        &self.name
    }
}
