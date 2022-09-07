use super::argument::ArgumentParameterType;
use super::mapped_arena::SerializableSlab;
use super::order::OrderByParameterType;
use super::predicate::PredicateParameterType;
use super::service::Script;
use super::service::ServiceMethod;
use super::ContextType;
use super::{
    mapped_arena::MappedArena,
    operation::{Mutation, Query},
};

use payas_sql::PhysicalTable;

use super::types::GqlType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSystem {
    pub types: SerializableSlab<GqlType>,
    pub contexts: MappedArena<ContextType>,

    // query related
    pub argument_types: SerializableSlab<ArgumentParameterType>,
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub queries: MappedArena<Query>,

    // mutation related
    pub mutation_types: SerializableSlab<GqlType>, // create, update, delete input types such as `PersonUpdateInput`
    pub mutations: MappedArena<Mutation>,

    // service related
    pub methods: SerializableSlab<ServiceMethod>,
    pub scripts: SerializableSlab<Script>,

    pub tables: SerializableSlab<PhysicalTable>,
}

impl Default for ModelSystem {
    fn default() -> Self {
        ModelSystem {
            types: SerializableSlab::new(),
            contexts: MappedArena::default(),
            order_by_types: SerializableSlab::new(),
            predicate_types: SerializableSlab::new(),
            queries: MappedArena::default(),
            mutation_types: SerializableSlab::new(),
            mutations: MappedArena::default(),
            tables: SerializableSlab::new(),
            methods: SerializableSlab::new(),
            argument_types: SerializableSlab::new(),
            scripts: SerializableSlab::new(),
        }
    }
}
