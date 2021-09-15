use super::{operation_context::OperationContext, sql_mapper::SQLMapper};
use anyhow::*;
use async_graphql_value::Value;
use payas_model::{
    model::limit_offset::{LimitParameter, OffsetParameter},
    sql::{Limit, Offset},
};

fn cast_to_i64(argument: &Value) -> Result<i64> {
    match argument {
        Value::Number(n) => Ok(n.as_i64().ok_or_else(|| anyhow!("Could not cast {} to i64", n))?),
        _ => Err(anyhow!("Not a number")),
    }
}

impl<'a> SQLMapper<'a, Limit> for LimitParameter {
    fn map_to_sql(
        &self,
        argument: &'a Value,
        _operation_context: &'a OperationContext<'a>,
    ) -> Result<Limit> {
        cast_to_i64(argument).map(Limit)
    }
}

impl<'a> SQLMapper<'a, Offset> for OffsetParameter {
    fn map_to_sql(
        &self,
        argument: &'a Value,
        _operation_context: &'a OperationContext<'a>,
    ) -> Result<Offset> {
        cast_to_i64(argument).map(Offset)
    }
}
