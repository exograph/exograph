// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use chrono::FixedOffset;
#[cfg(feature = "bigdecimal")]
use pg_bigdecimal::{BigDecimal, PgNumeric};
use std::{
    fmt::{Debug, Display},
    sync::Arc,
};
use tokio_postgres::types::{to_sql_checked, ToSql, Type};

use crate::{SQLBytes, SQLParam};

use super::{physical_column::to_pg_array_type, sql_param::SQLParamWithType, SQLValue};

#[derive(Clone)]
pub struct SQLParamContainer(SQLParamWithType);

impl SQLParamContainer {
    pub fn param(&self) -> SQLParamWithType {
        self.0.clone()
    }
}

impl ToSql for SQLParamContainer {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        self.0 .0.as_ref().to_sql_checked(ty, out)
    }

    fn accepts(_ty: &Type) -> bool {
        true // TODO: Can we check this?
    }

    to_sql_checked!();
}

impl SQLParamContainer {
    pub fn new<T: SQLParam + 'static>(param: T, param_type: Type) -> Self {
        Self((Arc::new(param), param_type, false))
    }

    pub fn string(value: String) -> Self {
        Self::new(value, Type::TEXT)
    }

    pub fn str(value: &'static str) -> Self {
        Self::new(value, Type::TEXT)
    }

    pub fn bool(value: bool) -> Self {
        Self::new(value, Type::BOOL)
    }

    pub fn i16(value: i16) -> Self {
        Self::new(value, Type::INT2)
    }

    pub fn i32(value: i32) -> Self {
        Self::new(value, Type::INT4)
    }

    pub fn i64(value: i64) -> Self {
        Self::new(value, Type::INT8)
    }

    pub fn f32(value: f32) -> Self {
        Self::new(value, Type::FLOAT4)
    }

    pub fn f64(value: f64) -> Self {
        Self::new(value, Type::FLOAT8)
    }

    pub fn f32_array(value: Vec<f32>) -> Self {
        Self::new(value, Type::FLOAT4_ARRAY)
    }

    pub fn uuid(value: uuid::Uuid) -> Self {
        Self::new(value, Type::UUID)
    }

    pub fn bytes(value: bytes::Bytes) -> Self {
        Self::new(SQLBytes(value), Type::BYTEA)
    }

    pub fn bytes_from_vec(value: Vec<u8>) -> Self {
        Self::new(SQLBytes::new(value), Type::BYTEA)
    }

    #[cfg(feature = "bigdecimal")]
    pub fn numeric(decimal: Option<BigDecimal>) -> Self {
        Self::new(PgNumeric { n: decimal }, Type::NUMERIC)
    }

    pub fn date(value: chrono::NaiveDate) -> Self {
        Self::new(value, Type::DATE)
    }

    pub fn time(value: chrono::NaiveTime) -> Self {
        Self::new(value, Type::TIME)
    }

    pub fn timestamp(value: chrono::NaiveDateTime) -> Self {
        Self::new(value, Type::TIMESTAMP)
    }

    pub fn timestamp_tz(value: chrono::DateTime<FixedOffset>) -> Self {
        Self::new(value, Type::TIMESTAMPTZ)
    }

    pub fn timestamp_utc(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self::new(value, Type::TIMESTAMPTZ)
    }

    pub fn json(value: serde_json::Value) -> Self {
        Self::new(value, Type::JSONB)
    }

    pub fn string_array(value: Vec<String>) -> Self {
        Self::new(value, Type::TEXT_ARRAY)
    }

    pub fn from_sql_values(params: Vec<SQLValue>, elem_type: Type) -> Self {
        let collection_type = to_pg_array_type(&elem_type);

        Self::new(params, collection_type)
    }

    pub fn from_sql_value(value: SQLValue) -> Self {
        let type_ = value.type_.clone();
        Self::new(value, type_)
    }

    pub fn with_unnest(self) -> Self {
        Self((self.0 .0, self.0 .1, true))
    }

    pub fn has_unnest(&self) -> bool {
        self.0 .2
    }
}

impl PartialEq for SQLParamContainer {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Debug for SQLParamContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Display for SQLParamContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
