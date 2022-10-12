use async_graphql_value::ConstValue;

use payas_database_model::{
    limit_offset::{LimitParameter, OffsetParameter},
    model::ModelDatabaseSystem,
};
use payas_sql::{Limit, Offset};

use super::{database_execution_error::DatabaseExecutionError, sql_mapper::SQLMapper};

fn cast_to_i64(argument: &ConstValue) -> Result<i64, DatabaseExecutionError> {
    match argument {
        ConstValue::Number(n) => n
            .as_i64()
            .ok_or_else(|| DatabaseExecutionError::Generic(format!("Could not cast {} to i64", n))),
        _ => Err(DatabaseExecutionError::Generic("Not a number".into())),
    }
}

impl<'a> SQLMapper<'a, Limit> for LimitParameter {
    fn map_to_sql(
        &self,
        argument: &'a ConstValue,
        _subsystem: &'a ModelDatabaseSystem,
    ) -> Result<Limit, DatabaseExecutionError> {
        cast_to_i64(argument).map(Limit)
    }
}

impl<'a> SQLMapper<'a, Offset> for OffsetParameter {
    fn map_to_sql(
        &self,
        argument: &'a ConstValue,
        _subsystem: &'a ModelDatabaseSystem,
    ) -> Result<Offset, DatabaseExecutionError> {
        cast_to_i64(argument).map(Offset)
    }
}
