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
use chrono::{DateTime, NaiveDateTime, NaiveTime};
use common::value::Val;
use exo_sql::{PhysicalColumnType, SQLParamContainer, TimeColumnType};

const NAIVE_DATE_FORMAT: &str = "%Y-%m-%d";
const NAIVE_TIME_FORMAT: &str = "%H:%M:%S%.f";

pub struct TimeCastProvider;

impl CastProvider for TimeCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::String(_)) && destination_type.as_any().is::<TimeColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        _destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if let Val::String(string) = val {
            // Try parsing as RFC3339 datetime first
            if let Ok(datetime) = DateTime::parse_from_rfc3339(string) {
                return Ok(Some(SQLParamContainer::time(datetime.time())));
            }

            // Try parsing as naive datetime with T separator
            if let Ok(naive_datetime) = NaiveDateTime::parse_from_str(
                string,
                &format!("{NAIVE_DATE_FORMAT}T{NAIVE_TIME_FORMAT}"),
            ) {
                return Ok(Some(SQLParamContainer::time(naive_datetime.time())));
            }

            // Try parsing as time-only format
            let t = NaiveTime::parse_from_str(string, NAIVE_TIME_FORMAT).map_err(|e| {
                CastError::Date(
                    format!("Could not parse {string} as a valid time-only format"),
                    e,
                )
            })?;
            Ok(Some(SQLParamContainer::time(t)))
        } else {
            Err(CastError::Generic("Expected string value".into()))
        }
    }
}
