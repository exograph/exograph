// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::CastProvider;
use crate::cast::CastError;
use common::value::Val;
use exo_sql::{NumericColumnType, PhysicalColumnType, SQLParamContainer};

#[cfg(feature = "bigdecimal")]
use exo_sql::BigDecimal;
#[cfg(feature = "bigdecimal")]
use std::str::FromStr;

pub struct NumericCastProvider;

impl CastProvider for NumericCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::String(_) | Val::Number(_))
            && destination_type.as_any().is::<NumericColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        _destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        match val {
            Val::String(string) => {
                #[cfg(feature = "bigdecimal")]
                {
                    let decimal = match string.as_str() {
                        "NaN" => None,
                        _ => Some(BigDecimal::from_str(string).map_err(|_| {
                            CastError::Generic(format!("Could not parse {string} into a decimal"))
                        })?),
                    };
                    Ok(Some(SQLParamContainer::numeric(decimal)))
                }

                #[cfg(not(feature = "bigdecimal"))]
                {
                    return Err(CastError::Generic(
                        "Casting strings to decimal fields are not supported in this build".into(),
                    ));
                }
            }
            Val::Number(_) => Err(CastError::Generic(
                "Number literals cannot be specified for decimal fields (use string)".into(),
            )),
            _ => Err(CastError::Generic(
                "Unexpected value type for numeric".into(),
            )),
        }
    }
}
