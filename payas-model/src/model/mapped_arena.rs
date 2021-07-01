use std::{
    collections::{hash_map::Keys, HashMap},
    ops,
};

use id_arena::{Arena, Id};

#[derive(Debug, Clone)]
pub struct MappedArena<V> {
    pub values: Arena<V>,
    map: HashMap<String, Id<V>>,
}

impl<V> MappedArena<V> {
    pub fn keys(&self) -> Keys<String, Id<V>> {
        self.map.keys()
    }

    pub fn get_id(&self, key: &str) -> Option<Id<V>> {
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

    pub fn add(&mut self, key: &str, typ: V) -> Id<V> {
        let id = self.values.alloc(typ);
        self.map.insert(key.to_string(), id);
        id
    }

    pub fn iter(&self) -> id_arena::Iter<V, impl id_arena::ArenaBehavior> {
        self.values.iter()
    }

    pub fn iter_mut(&mut self) -> id_arena::IterMut<V, id_arena::DefaultArenaBehavior<V>> {
        self.values.iter_mut()
    }
}

// Needed for tests, should get DCEd for the main binary
pub fn sorted_values<V>(arena: &MappedArena<V>) -> Vec<&V> {
    let mut values = Vec::new();
    let mut keys = arena.keys().collect::<Vec<&String>>();
    keys.sort();
    for key in keys.iter() {
        values.push(arena.get_by_key(key).unwrap());
    }
    values
}

impl<V> Default for MappedArena<V> {
    fn default() -> Self {
        MappedArena {
            values: Arena::default(),
            map: HashMap::default(),
        }
    }
}

impl<T> ops::Index<Id<T>> for MappedArena<T> {
    type Output = T;

    #[inline]
    fn index(&self, id: Id<T>) -> &T {
        &self.values[id]
    }
}

impl<T> ops::IndexMut<Id<T>> for MappedArena<T> {
    #[inline]
    fn index_mut(&mut self, id: Id<T>) -> &mut T {
        &mut self.values[id]
    }
}
