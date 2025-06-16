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
use chrono::{DateTime, NaiveDateTime, Utc};
use common::value::Val;
use exo_sql::{PhysicalColumnType, SQLParamContainer, TimestampColumnType};

const NAIVE_DATE_FORMAT: &str = "%Y-%m-%d";
const NAIVE_TIME_FORMAT: &str = "%H:%M:%S%.f";

pub struct TimestampCastProvider;

impl CastProvider for TimestampCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::String(_)) && destination_type.as_any().is::<TimestampColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if let Val::String(string) = val {
            if let Some(timestamp_type) = destination_type
                .as_any()
                .downcast_ref::<TimestampColumnType>()
            {
                let timezone = &timestamp_type.timezone;

                // Try parsing as RFC3339 datetime first
                if let Ok(datetime) = DateTime::parse_from_rfc3339(string) {
                    if *timezone {
                        return Ok(Some(SQLParamContainer::timestamp_tz(datetime)));
                    } else {
                        return Ok(Some(SQLParamContainer::timestamp(datetime.naive_local())));
                    }
                }

                // Try parsing as naive datetime with T separator
                if let Ok(naive_datetime) = NaiveDateTime::parse_from_str(
                    string,
                    &format!("{NAIVE_DATE_FORMAT}T{NAIVE_TIME_FORMAT}"),
                ) {
                    if *timezone {
                        return Ok(Some(SQLParamContainer::timestamp_utc(
                            DateTime::<Utc>::from_naive_utc_and_offset(naive_datetime, chrono::Utc),
                        )));
                    } else {
                        return Ok(Some(SQLParamContainer::timestamp(naive_datetime)));
                    }
                }

                Err(CastError::Generic(format!(
                    "Could not parse {string} as a valid timestamp format"
                )))
            } else {
                Err(CastError::Generic("Expected TimestampColumnType".into()))
            }
        } else {
            Err(CastError::Generic("Expected string value".into()))
        }
    }
}
