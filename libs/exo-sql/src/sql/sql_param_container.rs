// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    fmt::{Debug, Display},
    sync::Arc,
};
use tokio_postgres::types::{to_sql_checked, ToSql, Type};

use crate::SQLParam;

/// Newtype for SQL parameters that can be used in a prepared statement. We would have been fine
/// with just using `Arc<dyn SQLParam>` but we need to implement `ToSql` for it and since `Arc`
/// (unlike `Box`) is not a `#[fundamental]` type, we have to wrap it in a newtype.
#[derive(Clone)]
pub struct SQLParamContainer(Arc<dyn SQLParam>);

impl SQLParamContainer {
    pub fn param(&self) -> Arc<dyn SQLParam> {
        self.0.clone()
    }
}

impl ToSql for SQLParamContainer {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        self.0.as_ref().to_sql_checked(ty, out)
    }

    fn accepts(_ty: &Type) -> bool {
        true // TODO: Can we check this?
    }

    to_sql_checked!();
}

impl SQLParamContainer {
    pub fn new<T: SQLParam + 'static>(param: T) -> Self {
        Self(Arc::new(param))
    }
}

impl PartialEq for SQLParamContainer {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl AsRef<dyn SQLParam> for SQLParamContainer {
    fn as_ref(&self) -> &(dyn SQLParam + 'static) {
        self.0.as_ref()
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
