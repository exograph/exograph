// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use crate::relation::PostgresRelation;
use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;

#[derive(Serialize, Deserialize, Debug)]
pub struct AggregateType {
    pub name: String, // Such as IntAgg, ConcertAgg.
    pub fields: Vec<AggregateField>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AggregateField {
    pub name: String, // Such as max, sum, etc for scalar types; field names (id, name, etc.) for composite types
    pub typ: AggregateFieldType,
    pub relation: Option<PostgresRelation>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum AggregateFieldType {
    Scalar {
        type_name: String,              // "Int", "String", etc.
        kind: ScalarAggregateFieldKind, // Min, Max, Sum, etc.
    },
    Composite {
        type_name: String,
        type_id: SerializableSlabIndex<AggregateType>,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum ScalarAggregateFieldKind {
    Avg,
    Count,
    Max,
    Min,
    Sum,
}

impl ScalarAggregateFieldKind {
    pub fn name(&self) -> &str {
        match self {
            ScalarAggregateFieldKind::Avg => "avg",
            ScalarAggregateFieldKind::Count => "count",
            ScalarAggregateFieldKind::Max => "max",
            ScalarAggregateFieldKind::Min => "min",
            ScalarAggregateFieldKind::Sum => "sum",
        }
    }
}
