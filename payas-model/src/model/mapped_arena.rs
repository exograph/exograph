use std::{
    collections::{hash_map::Keys, HashMap},
    ops,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use typed_generational_arena::{Arena, IgnoreGeneration, Index};

pub type SerializableSlab<T> = Arena<T, usize, IgnoreGeneration>;
pub type SerializableSlabIndex<T> = Index<T, usize, IgnoreGeneration>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MappedArena<V> {
    pub values: SerializableSlab<V>,
    map: HashMap<String, SerializableSlabIndex<V>>,
}

impl<V: DeserializeOwned + Serialize> MappedArena<V> {
    pub fn keys(&self) -> Keys<String, SerializableSlabIndex<V>> {
        self.map.keys()
    }

    pub fn get_id(&self, key: &str) -> Option<SerializableSlabIndex<V>> {
        self.map.get(key).copied()
    }

    pub fn get_by_key(&self, key: &str) -> Option<&V> {
        self.get_id(key).map(|id| &self[id])
    }

    pub fn get_by_key_mut(&mut self, key: &str) -> Option<&mut V> {
        #[allow(clippy::manual_map)]
        if let Some(id) = self.get_id(key) {
            Some(&mut self[id])
        } else {
            None
        }
    }

    pub fn add(&mut self, key: &str, typ: V) -> SerializableSlabIndex<V> {
        let id = self.values.insert(typ);
        self.map.insert(key.to_string(), id);
        id
    }

    pub fn iter(&self) -> typed_generational_arena::Iter<V, usize, IgnoreGeneration> {
        self.values.iter()
    }

    pub fn iter_mut(&mut self) -> typed_generational_arena::IterMut<V, usize, IgnoreGeneration> {
        self.values.iter_mut()
    }
}

// Needed for tests, should get DCEd for the main binary
pub fn sorted_values<V: DeserializeOwned + Serialize>(arena: &MappedArena<V>) -> Vec<&V> {
    let mut values = Vec::new();
    let mut keys = arena.keys().collect::<Vec<&String>>();
    keys.sort();
    for key in keys.iter() {
        values.push(arena.get_by_key(key).unwrap());
    }
    values
}

impl<V: DeserializeOwned + Serialize> Default for MappedArena<V> {
    fn default() -> Self {
        MappedArena {
            values: SerializableSlab::new(),
            map: HashMap::default(),
        }
    }
}

impl<T: DeserializeOwned + Serialize> ops::Index<SerializableSlabIndex<T>> for MappedArena<T> {
    type Output = T;

    #[inline]
    fn index(&self, id: SerializableSlabIndex<T>) -> &T {
        &self.values[id]
    }
}

impl<T: DeserializeOwned + Serialize> ops::IndexMut<SerializableSlabIndex<T>> for MappedArena<T> {
    #[inline]
    fn index_mut(&mut self, id: SerializableSlabIndex<T>) -> &mut T {
        &mut self.values[id]
    }
}
