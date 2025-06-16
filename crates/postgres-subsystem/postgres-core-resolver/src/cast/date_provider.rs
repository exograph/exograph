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
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use common::value::Val;
use exo_sql::{DateColumnType, PhysicalColumnType, SQLParamContainer};

const NAIVE_DATE_FORMAT: &str = "%Y-%m-%d";
const NAIVE_TIME_FORMAT: &str = "%H:%M:%S%.f";

pub struct DateCastProvider;

impl CastProvider for DateCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::String(_)) && destination_type.as_any().is::<DateColumnType>()
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
                return Ok(Some(SQLParamContainer::date(datetime.date_naive())));
            }

            // Try parsing as naive datetime with T separator
            if let Ok(naive_datetime) = NaiveDateTime::parse_from_str(
                string,
                &format!("{NAIVE_DATE_FORMAT}T{NAIVE_TIME_FORMAT}"),
            ) {
                return Ok(Some(SQLParamContainer::date(naive_datetime.date())));
            }

            // Try parsing as date-only format
            let d = NaiveDate::parse_from_str(string, NAIVE_DATE_FORMAT).map_err(|e| {
                CastError::Date(
                    format!("Could not parse {string} as a valid date-only format"),
                    e,
                )
            })?;
            Ok(Some(SQLParamContainer::date(d)))
        } else {
            Err(CastError::Generic("Expected a date string".into()))
        }
    }
}
