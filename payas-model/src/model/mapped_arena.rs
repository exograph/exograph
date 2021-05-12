use std::{collections::HashMap, ops};

use id_arena::{Arena, Id};

#[derive(Debug, Clone)]
pub struct MappedArena<V> {
    pub values: Arena<V>,
    map: HashMap<String, Id<V>>,
}

impl<V> MappedArena<V> {
    pub fn new() -> MappedArena<V> {
        MappedArena {
            values: Arena::new(),
            map: HashMap::new(),
        }
    }

    pub fn get_id(&self, key: &str) -> Option<Id<V>> {
        self.map.get(key).copied()
    }

    pub fn get_by_key(&self, key: &str) -> Option<&V> {
        self.get_id(key).map(|id| &self[id])
    }

    pub fn add(&mut self, key: &str, typ: V) -> Id<V> {
        let id = self.values.alloc(typ);
        self.map.insert(key.to_string(), id);
        id
    }

    pub fn iter(&self) -> id_arena::Iter<V, impl id_arena::ArenaBehavior> {
        self.values.iter()
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
