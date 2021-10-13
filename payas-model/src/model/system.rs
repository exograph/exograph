use super::argument::ArgumentParameterType;
use super::mapped_arena::SerializableSlab;
use super::order::*;
use super::predicate::*;
use super::service::ServiceMethod;
use super::ContextType;
use super::{mapped_arena::MappedArena, operation::*};

use crate::sql::PhysicalTable;

use super::types::GqlType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSystem {
    pub types: SerializableSlab<GqlType>,
    pub contexts: SerializableSlab<ContextType>,
    pub argument_types: SerializableSlab<ArgumentParameterType>,
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: SerializableSlab<GqlType>,
    pub create_mutations: MappedArena<Mutation>,
    pub tables: SerializableSlab<PhysicalTable>,
    pub methods: SerializableSlab<ServiceMethod>,
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
            methods: SerializableSlab::new(),
            argument_types: SerializableSlab::new(),
        }
    }
}
