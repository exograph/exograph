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

/// Trait that all physical column types must implement
pub trait PhysicalColumnType: Send + Sync + Debug {
    /// Returns a string description of the type
    fn type_string(&self) -> String;

    /// Returns the type name for serialization
    fn type_name(&self) -> &'static str;

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Clone the type as a boxed trait object
    fn clone_box(&self) -> Box<dyn PhysicalColumnType>;

    /// Check equality with another physical column type
    fn equals(&self, other: &dyn PhysicalColumnType) -> bool;
}

// Implement standard traits directly on Box<dyn PhysicalColumnType>
impl Clone for Box<dyn PhysicalColumnType> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl PartialEq for Box<dyn PhysicalColumnType> {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other.as_ref())
    }
}

impl Eq for Box<dyn PhysicalColumnType> {}

// Extension trait to add helper methods to Box<dyn PhysicalColumnType>
pub trait PhysicalColumnTypeExt {
    /// Check if the inner type is of a specific type
    fn is<T: 'static>(&self) -> bool;

    /// Get a reference to the inner type (for compatibility)
    fn inner(&self) -> &dyn PhysicalColumnType;
}

impl PhysicalColumnTypeExt for Box<dyn PhysicalColumnType> {
    fn is<T: 'static>(&self) -> bool {
        self.as_ref().as_any().is::<T>()
    }

    fn inner(&self) -> &dyn PhysicalColumnType {
        self.as_ref()
    }
}

// Trait for serializing and deserializing physical column types
pub trait PhysicalColumnTypeSerializer: Send + Sync {
    fn serialize(&self, column_type: &dyn PhysicalColumnType) -> Result<Vec<u8>, String>;
    fn deserialize(&self, data: &[u8]) -> Result<Box<dyn PhysicalColumnType>, String>;
}

// Global registry for physical column types (initialized by pg-core)
static PHYSICAL_COLUMN_TYPE_REGISTRY: OnceLock<
    IndexMap<&'static str, Box<dyn PhysicalColumnTypeSerializer>>,
> = OnceLock::new();

/// Set the physical column type registry. Called by database-specific crates at initialization.
pub fn set_physical_column_type_registry(
    registry: IndexMap<&'static str, Box<dyn PhysicalColumnTypeSerializer>>,
) {
    // ok() is intentional: if already set (e.g., concurrent init), the first registration wins.
    // This is safe because all callers register the same set of types.
    PHYSICAL_COLUMN_TYPE_REGISTRY.set(registry).ok();
}

pub fn get_physical_column_type_registry()
-> &'static IndexMap<&'static str, Box<dyn PhysicalColumnTypeSerializer>> {
    PHYSICAL_COLUMN_TYPE_REGISTRY
        .get()
        .expect("Physical column type registry not initialized. Ensure pg-core is initialized.")
}

// Serialization wrapper
#[derive(Serialize, Deserialize)]
pub struct SerializedPhysicalColumnType {
    pub type_name: String,
    pub data: Vec<u8>,
}

impl Serialize for Box<dyn PhysicalColumnType> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let type_name = self.type_name();

        let registry = get_physical_column_type_registry();
        let entry = registry.get(type_name).ok_or_else(|| {
            serde::ser::Error::custom(format!("Unknown physical column type: {}", type_name))
        })?;

        let data = entry
            .serialize(self.as_ref())
            .map_err(serde::ser::Error::custom)?;

        let wrapper = SerializedPhysicalColumnType {
            type_name: type_name.to_string(),
            data,
        };

        wrapper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Box<dyn PhysicalColumnType> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wrapper = SerializedPhysicalColumnType::deserialize(deserializer)?;

        let registry = get_physical_column_type_registry();
        let entry = registry.get(wrapper.type_name.as_str()).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "Unknown physical column type: {}",
                wrapper.type_name
            ))
        })?;

        entry
            .deserialize(&wrapper.data)
            .map_err(serde::de::Error::custom)
    }
}
