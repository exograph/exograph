// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{any::Any, sync::Arc};

use crate::SchemaObjectName;
use tokio_postgres::types::{ToSql, Type};

#[derive(Debug, Clone)]
pub struct SQLParamWithType {
    pub param: Arc<dyn SQLParam>,
    pub param_type: Type,
    pub is_array: bool,
    pub enum_type: Option<SchemaObjectName>,
}

/// A trait to simplify our use of SQL parameters, specifically to have the [Send] and [Sync] bounds.
pub trait SQLParam: ToSql + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn SQLParam) -> bool;

    /// Create type-compatible version that we can pass to the Postgres driver.
    fn as_pg(&self) -> &(dyn ToSql + Sync);
}

impl<T: ToSql + Send + Sync + Any + PartialEq> SQLParam for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn SQLParam) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<T>() {
            self == other
        } else {
            false
        }
    }

    fn as_pg(&self) -> &(dyn ToSql + Sync) {
        self
    }
}

impl PartialEq for dyn SQLParam {
    fn eq(&self, other: &Self) -> bool {
        SQLParam::eq(self, other)
    }
}
