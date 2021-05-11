use std::{collections::HashMap, ops};

use id_arena::{Arena, Id};

use crate::sql::PhysicalTable;

use super::{
    operation::{Mutation, Query},
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    types::ModelType,
};

#[derive(Debug, Clone)]
pub struct MappedArena<V> {
    pub values: Arena<V>,
    map: HashMap<String, Id<V>>,
}

impl<V> MappedArena<V> {
    fn new() -> MappedArena<V> {
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

#[derive(Debug)]
pub struct SystemContextBuilding {
    pub types: MappedArena<ModelType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: MappedArena<ModelType>,
    pub mutations: MappedArena<Mutation>,
    pub tables: MappedArena<PhysicalTable>,
}

impl SystemContextBuilding {
    pub fn new() -> Self {
        Self {
            types: MappedArena::new(),
            order_by_types: MappedArena::new(),
            predicate_types: MappedArena::new(),
            queries: MappedArena::new(),
            mutation_types: MappedArena::new(),
            mutations: MappedArena::new(),
            tables: MappedArena::new(),
        }
    }
}

// mod tests {
//     use super::*;

//     #[test]
//     fn basic() {
//         let ast_type = AstType {
//             name: "people".to_string(),
//             kind: AstTypeKind::Composite {
//                 fields: vec![AstField {
//                     name: "id".to_string(),
//                     type_name: "Int".to_string(),
//                     type_modifier: AstTypeModifier::NonNull,
//                     relation: AstRelation::Pk { column_name: None },
//                 }],
//                 table_name: None,
//             },
//         };

//         let context = SystemContext::build(&[ast_type]);
//         dbg!(context);
//     }
// }
