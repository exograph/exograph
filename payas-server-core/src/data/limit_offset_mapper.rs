use crate::execution::system_context::SystemContext;

use super::operation_mapper::SQLMapper;
use anyhow::{anyhow, Result};
use async_graphql_value::ConstValue;
use payas_model::model::limit_offset::{LimitParameter, OffsetParameter};
use payas_sql::{Limit, Offset};

fn cast_to_i64(argument: &ConstValue) -> Result<i64> {
    match argument {
        ConstValue::Number(n) => Ok(n
            .as_i64()
            .ok_or_else(|| anyhow!("Could not cast {} to i64", n))?),
        _ => Err(anyhow!("Not a number")),
    }
}

impl<'a> SQLMapper<'a, Limit> for LimitParameter {
    fn map_to_sql(
        &self,
        argument: &'a ConstValue,
        _system_context: &'a SystemContext,
    ) -> Result<Limit> {
        cast_to_i64(argument).map(Limit)
    }
}

impl<'a> SQLMapper<'a, Offset> for OffsetParameter {
    fn map_to_sql(
        &self,
        argument: &'a ConstValue,
        _system_context: &'a SystemContext,
    ) -> Result<Offset> {
        cast_to_i64(argument).map(Offset)
    }
}
