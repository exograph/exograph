// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! A wrapper around a `typed_generational_arena::Arena` that also provides fast lookup.
//!
//! We use `MappedArena` to store queries, mutations, and contexts. In each of these cases, we need
//! to lookup the underlying object given a key. For example, the key would be the name of the query
//! or mutation (such as `concerts` or `deleteConcert`).
//!
//! We need a fast lookup during resolve time. For example, if we are resolving a query, we need to
//! find the `Query` object associated with that name. If we don't maintain a map, we would have to
//! a linear search.

use std::{
    collections::{HashMap, hash_map::Keys},
    ops,
};

use serde::{Deserialize, Serialize};

use typed_generational_arena::{Arena, IgnoreGeneration, Index};

pub type SerializableSlab<T> = Arena<T, usize, IgnoreGeneration>;
pub type SerializableSlabIndex<T> = Index<T, usize, IgnoreGeneration>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MappedArena<V> {
    values: SerializableSlab<V>,
    map: HashMap<String, SerializableSlabIndex<V>>,
}

impl<V> MappedArena<V> {
    pub fn values(self) -> SerializableSlab<V> {
        self.values
    }

    pub fn values_ref(&self) -> &SerializableSlab<V> {
        &self.values
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn keys(&self) -> Keys<'_, String, SerializableSlabIndex<V>> {
        self.map.keys()
    }

    pub fn get_id(&self, key: &str) -> Option<SerializableSlabIndex<V>> {
        self.map.get(key).copied()
    }

    pub fn get_by_key(&self, key: &str) -> Option<&V> {
        self.get_id(key).map(|id| &self[id])
    }

    pub fn get_by_key_mut(&mut self, key: &str) -> Option<&mut V> {
        if let Some(id) = self.get_id(key) {
            Some(&mut self[id])
        } else {
            None
        }
    }

    pub fn get_by_id_mut(&mut self, id: SerializableSlabIndex<V>) -> &mut V {
        &mut self.values[id]
    }

    pub fn add(&mut self, key: &str, typ: V) -> SerializableSlabIndex<V> {
        let existing = self.get_id(key);
        if let Some(existing) = existing {
            return existing;
        }

        let id = self.values.insert(typ);
        self.map.insert(key.to_string(), id);
        id
    }

    pub fn iter(&self) -> typed_generational_arena::Iter<'_, V, usize, IgnoreGeneration> {
        self.values.iter()
    }
}

impl<V> Default for MappedArena<V> {
    fn default() -> Self {
        MappedArena {
            values: SerializableSlab::new(),
            map: HashMap::default(),
        }
    }
}

impl<V> ops::Index<SerializableSlabIndex<V>> for MappedArena<V> {
    type Output = V;

    #[inline]
    fn index(&self, id: SerializableSlabIndex<V>) -> &V {
        &self.values[id]
    }
}

impl<V> ops::IndexMut<SerializableSlabIndex<V>> for MappedArena<V> {
    #[inline]
    fn index_mut(&mut self, id: SerializableSlabIndex<V>) -> &mut V {
        &mut self.values[id]
    }
}
