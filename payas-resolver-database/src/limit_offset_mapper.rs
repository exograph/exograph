use async_graphql_value::ConstValue;

use payas_model::model::limit_offset::{LimitParameter, OffsetParameter};
use payas_sql::{Limit, Offset};

use super::{
    database_execution_error::DatabaseExecutionError,
    database_system_context::DatabaseSystemContext, sql_mapper::SQLMapper,
};

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
        _system_context: &DatabaseSystemContext<'a>,
    ) -> Result<Limit, DatabaseExecutionError> {
        cast_to_i64(argument).map(Limit)
    }
}

impl<'a> SQLMapper<'a, Offset> for OffsetParameter {
    fn map_to_sql(
        &self,
        argument: &'a ConstValue,
        _system_context: &DatabaseSystemContext<'a>,
    ) -> Result<Offset, DatabaseExecutionError> {
        cast_to_i64(argument).map(Offset)
    }
}
