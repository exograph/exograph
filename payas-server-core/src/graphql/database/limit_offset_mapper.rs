use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};

use super::sql_mapper::SQLMapper;
use async_graphql_value::ConstValue;
use payas_model::model::limit_offset::{LimitParameter, OffsetParameter};
use payas_sql::{Limit, Offset};

fn cast_to_i64(argument: &ConstValue) -> Result<i64, ExecutionError> {
    match argument {
        ConstValue::Number(n) => n
            .as_i64()
            .ok_or_else(|| ExecutionError::Generic(format!("Could not cast {} to i64", n))),
        _ => Err(ExecutionError::Generic("Not a number".into())),
    }
}

impl<'a> SQLMapper<'a, Limit> for LimitParameter {
    fn map_to_sql(
        &self,
        argument: &'a ConstValue,
        _system_context: &'a SystemContext,
    ) -> Result<Limit, ExecutionError> {
        cast_to_i64(argument).map(Limit)
    }
}

impl<'a> SQLMapper<'a, Offset> for OffsetParameter {
    fn map_to_sql(
        &self,
        argument: &'a ConstValue,
        _system_context: &'a SystemContext,
    ) -> Result<Offset, ExecutionError> {
        cast_to_i64(argument).map(Offset)
    }
}
