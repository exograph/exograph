// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Postgres-specific index kind types, used by both pg and pg-schema crates.

use crate::core::vector::VectorDistanceFunction;
use exo_sql_core::DefaultIndexKind;
use exo_sql_core::index_kind::{PhysicalIndexKind, PhysicalIndexKindSerializer};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::sync::LazyLock;

/// Postgres-specific index kind, covering HNSW (pgvector) and database-default indices.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum IndexKind {
    HNWS {
        distance_function: VectorDistanceFunction,
        params: Option<HNWSParams>,
    },
    #[default]
    DatabaseDefault,
}

/// Parameters for the HNSW index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct HNWSParams {
    pub m: usize,
    pub ef_construction: usize,
}

impl PhysicalIndexKind for IndexKind {
    fn type_name(&self) -> &'static str {
        "PgIndexKind"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn PhysicalIndexKind> {
        Box::new(self.clone())
    }

    fn equals(&self, other: &dyn PhysicalIndexKind) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|o| o == self)
    }
}

/// Serializer for pg `IndexKind`.
pub struct PgIndexKindSerializer;

impl PhysicalIndexKindSerializer for PgIndexKindSerializer {
    fn serialize(&self, index_kind: &dyn PhysicalIndexKind) -> Result<Vec<u8>, String> {
        index_kind
            .as_any()
            .downcast_ref::<IndexKind>()
            .ok_or_else(|| "Expected PgIndexKind".to_string())
            .and_then(|t| {
                postcard::to_allocvec(t)
                    .map_err(|e| format!("Failed to serialize PgIndexKind: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalIndexKind>, String> {
        let (t, remaining) = postcard::take_from_bytes::<IndexKind>(data)
            .map_err(|e| format!("Failed to deserialize PgIndexKind: {}", e))?;
        if !remaining.is_empty() {
            return Err(
                "Did not consume all bytes during deserialization of PgIndexKind".to_string(),
            );
        }
        Ok(Box::new(t))
    }
}

/// Serializer for `DefaultIndexKind`.
struct DefaultIndexKindSerializer;

impl PhysicalIndexKindSerializer for DefaultIndexKindSerializer {
    fn serialize(&self, index_kind: &dyn PhysicalIndexKind) -> Result<Vec<u8>, String> {
        index_kind
            .as_any()
            .downcast_ref::<DefaultIndexKind>()
            .ok_or_else(|| "Expected DefaultIndexKind".to_string())
            .and_then(|t| {
                postcard::to_allocvec(t)
                    .map_err(|e| format!("Failed to serialize DefaultIndexKind: {}", e))
            })
    }

    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalIndexKind>, String> {
        let (t, remaining) = postcard::take_from_bytes::<DefaultIndexKind>(data)
            .map_err(|e| format!("Failed to deserialize DefaultIndexKind: {}", e))?;
        if !remaining.is_empty() {
            return Err(
                "Did not consume all bytes during deserialization of DefaultIndexKind".to_string(),
            );
        }
        Ok(Box::new(t))
    }
}

// -- Registry initialization --

static INDEX_KIND_REGISTRY_INIT: LazyLock<()> = LazyLock::new(|| {
    let mut registry: IndexMap<&'static str, Box<dyn PhysicalIndexKindSerializer>> =
        IndexMap::new();

    registry.insert(
        "Default",
        Box::new(DefaultIndexKindSerializer) as Box<dyn PhysicalIndexKindSerializer>,
    );
    registry.insert(
        "PgIndexKind",
        Box::new(PgIndexKindSerializer) as Box<dyn PhysicalIndexKindSerializer>,
    );

    exo_sql_core::index_kind::set_physical_index_kind_registry(registry);
});

/// Ensure the physical index kind registry is initialized.
/// Must be called before any serialization/deserialization of PhysicalIndex.
pub fn ensure_index_kind_registry_initialized() {
    LazyLock::force(&INDEX_KIND_REGISTRY_INIT);
}
