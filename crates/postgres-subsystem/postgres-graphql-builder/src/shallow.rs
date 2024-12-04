// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
use postgres_core_model::types::TypeIndex;

/// A trait that allows a default-like value during shallow building
///
/// It is expected that the shallow value will be replaced with a real value
/// during the expansion phase.
pub trait Shallow {
    fn shallow() -> Self;
}

impl<T> Shallow for SerializableSlabIndex<T> {
    fn shallow() -> Self {
        // Use an impossible index to make sure we don't accidentally use this (or if we use, it will panic)
        SerializableSlabIndex::from_idx(usize::MAX)
    }
}

impl<CT> Shallow for TypeIndex<CT> {
    fn shallow() -> Self {
        TypeIndex::Primitive(SerializableSlabIndex::shallow())
    }
}
