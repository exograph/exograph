// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

/// Built-in projection name: primary key fields only. Default for mutations.
pub const PROJECTION_PK: &str = "pk";
/// Built-in projection name: all scalars + ManyToOne as PK refs. Default for queries.
pub const PROJECTION_BASIC: &str = "basic";

/// A resolved projection — the concrete set of fields to include in a response.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResolvedProjection {
    pub name: String,
    pub elements: Vec<ProjectionElement>,
}

/// An element in a resolved projection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProjectionElement {
    /// A scalar field of this entity (field name).
    ScalarField(String),
    /// A relation with a named projection applied.
    RelationProjection {
        relation_field_name: String,
        projection_name: String,
    },
}
