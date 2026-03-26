// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt::Debug;
use std::sync::OnceLock;

/// Trait that all physical index kinds must implement.
///
/// Follows the same trait-object pattern as `PhysicalColumnType` so that
/// `PhysicalIndex` and `PhysicalTable` remain non-generic while backends
/// define their own index kinds (e.g., HNSW for pgvector).
pub trait PhysicalIndexKind: Send + Sync + Debug {
    /// Returns the type name for serialization dispatch.
    fn type_name(&self) -> &'static str;

    /// Convert to Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Clone the index kind as a boxed trait object.
    fn clone_box(&self) -> Box<dyn PhysicalIndexKind>;

    /// Check equality with another index kind.
    fn equals(&self, other: &dyn PhysicalIndexKind) -> bool;
}

impl Clone for Box<dyn PhysicalIndexKind> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl PartialEq for Box<dyn PhysicalIndexKind> {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other.as_ref())
    }
}

impl Eq for Box<dyn PhysicalIndexKind> {}

/// Default index kind (database-default, e.g., btree).
///
/// Provided in core so that backends don't need to define their own
/// type for the common "just use the database default" case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultIndexKind;

impl PhysicalIndexKind for DefaultIndexKind {
    fn type_name(&self) -> &'static str {
        "Default"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn PhysicalIndexKind> {
        Box::new(self.clone())
    }

    fn equals(&self, other: &dyn PhysicalIndexKind) -> bool {
        other.as_any().downcast_ref::<Self>().is_some()
    }
}

// -- Serialization infrastructure (same pattern as PhysicalColumnType) --

/// Trait for serializing and deserializing physical index kinds.
pub trait PhysicalIndexKindSerializer: Send + Sync {
    fn serialize(&self, index_kind: &dyn PhysicalIndexKind) -> Result<Vec<u8>, String>;
    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalIndexKind>, String>;
}

static PHYSICAL_INDEX_KIND_REGISTRY: OnceLock<
    IndexMap<&'static str, Box<dyn PhysicalIndexKindSerializer>>,
> = OnceLock::new();

/// Set the physical index kind registry. Called by database-specific crates at initialization.
pub fn set_physical_index_kind_registry(
    registry: IndexMap<&'static str, Box<dyn PhysicalIndexKindSerializer>>,
) {
    PHYSICAL_INDEX_KIND_REGISTRY.set(registry).ok();
}

pub fn get_physical_index_kind_registry()
-> &'static IndexMap<&'static str, Box<dyn PhysicalIndexKindSerializer>> {
    PHYSICAL_INDEX_KIND_REGISTRY
        .get()
        .expect("Physical index kind registry not initialized. Ensure pg-core is initialized.")
}

/// Serialization wrapper for `Box<dyn PhysicalIndexKind>`.
#[derive(Serialize, Deserialize)]
struct SerializedPhysicalIndexKind {
    type_name: String,
    data: Vec<u8>,
}

impl Serialize for Box<dyn PhysicalIndexKind> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let type_name = self.type_name();

        let registry = get_physical_index_kind_registry();
        let entry = registry.get(type_name).ok_or_else(|| {
            serde::ser::Error::custom(format!("Unknown physical index kind: {}", type_name))
        })?;

        let data = entry
            .serialize(self.as_ref())
            .map_err(serde::ser::Error::custom)?;

        let wrapper = SerializedPhysicalIndexKind {
            type_name: type_name.to_string(),
            data,
        };

        wrapper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Box<dyn PhysicalIndexKind> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wrapper = SerializedPhysicalIndexKind::deserialize(deserializer)?;

        let registry = get_physical_index_kind_registry();
        let entry = registry.get(wrapper.type_name.as_str()).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "Unknown physical index kind: {}",
                wrapper.type_name
            ))
        })?;

        entry
            .deserialize(&wrapper.data)
            .map_err(serde::de::Error::custom)
    }
}
