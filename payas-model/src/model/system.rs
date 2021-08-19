use super::mapped_arena::SerializableSlab;
use super::order::*;
use super::predicate::*;
use super::ContextType;
use super::{mapped_arena::MappedArena, operation::*};

use crate::sql::PhysicalTable;

use super::types::GqlType;

#[derive(Debug, Clone)]
pub struct ModelSystem {
    pub types: SerializableSlab<GqlType>,
    pub contexts: SerializableSlab<ContextType>,
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: SerializableSlab<GqlType>,
    pub create_mutations: MappedArena<Mutation>,
    pub tables: SerializableSlab<PhysicalTable>,
}

impl Default for ModelSystem {
    fn default() -> Self {
        ModelSystem {
            types: SerializableSlab::new(),
            contexts: SerializableSlab::new(),
            order_by_types: SerializableSlab::new(),
            predicate_types: SerializableSlab::new(),
            queries: MappedArena::default(),
            mutation_types: SerializableSlab::new(),
            create_mutations: MappedArena::default(),
            tables: SerializableSlab::new(),
        }
    }
}
