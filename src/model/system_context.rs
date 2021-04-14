use std::collections::HashMap;

use id_arena::{Arena, Id};

use crate::sql::table::PhysicalTable;

use super::{
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    types::{ModelType, ModelTypeKind},
};

#[derive(Debug, Clone)]
pub struct MappedArena<V> {
    pub values: Arena<V>,
    pub map: HashMap<String, Id<V>>,
}

impl<V> MappedArena<V> {
    fn new() -> MappedArena<V> {
        MappedArena {
            values: Arena::new(),
            map: HashMap::new(),
        }
    }

    pub fn get_id(&self, key: &str) -> Option<Id<V>> {
        self.map.get(key).map(|id| *id)
    }

    pub fn get_by_key(&self, key: &str) -> Option<&V> {
        self.get_id(key).and_then(|id| self.get_by_id(id))
    }

    pub fn get_by_id(&self, id: Id<V>) -> Option<&V> {
        self.values.get(id)
    }

    pub fn get_by_id_mut(&mut self, id: Id<V>) -> Option<&mut V> {
        self.values.get_mut(id)
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

#[derive(Debug)]
pub struct SystemContextBuilding {
    pub types: MappedArena<ModelType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,
    pub tables: MappedArena<PhysicalTable>,
}

impl SystemContextBuilding {
    pub fn new() -> Self {
        Self {
            types: MappedArena::new(),
            order_by_types: MappedArena::new(),
            predicate_types: MappedArena::new(),
            tables: MappedArena::new(),
        }
    }

    pub fn update_type(&mut self, existing_id: Id<ModelType>, kind: ModelTypeKind) {
        self.types.get_by_id_mut(existing_id).map(|typ| {
            typ.kind = kind;
        });
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
