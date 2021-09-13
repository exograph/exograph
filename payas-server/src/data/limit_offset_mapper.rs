use super::{operation_context::OperationContext, sql_mapper::SQLMapper};
use anyhow::*;
use async_graphql_value::Value;
use payas_model::{
    model::limit_offset::{LimitParameter, OffsetParameter},
    sql::{Limit, Offset},
};

impl<'a> SQLMapper<'a, Limit> for LimitParameter {
    fn map_to_sql(
        &self,
        argument: &'a Value,
        _operation_context: &'a OperationContext<'a>,
    ) -> Result<Limit> {
        match argument {
            Value::Number(n) => Ok(Limit(n.as_i64().ok_or(anyhow!(""))?)),

            _ => Err(anyhow!("Not a number")),
        }
    }
}

impl<'a> SQLMapper<'a, Offset> for OffsetParameter {
    fn map_to_sql(
        &self,
        argument: &'a Value,
        _operation_context: &'a OperationContext<'a>,
    ) -> Result<Offset> {
        match argument {
            Value::Number(n) => Ok(Offset(n.as_i64().ok_or(anyhow!(""))?)),

            _ => Err(anyhow!("Not a number")),
        }
    } // FIXME: dedup
}
