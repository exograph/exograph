use crate::PhysicalTable;

use serde::{Deserialize, Serialize};
use typed_generational_arena::{Arena, IgnoreGeneration, Index};

pub type SerializableSlab<T> = Arena<T, usize, IgnoreGeneration>;
pub type SerializableSlabIndex<T> = Index<T, usize, IgnoreGeneration>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Database {
    pub tables: SerializableSlab<PhysicalTable>,
}

impl Default for Database {
    fn default() -> Self {
        Database {
            tables: SerializableSlab::new(),
        }
    }
}
