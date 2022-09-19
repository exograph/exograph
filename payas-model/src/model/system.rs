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
    pub primitive_types: SerializableSlab<GqlType>,
    pub database_types: SerializableSlab<GqlType>,
    // TODO: Break this up into deno/wasm
    pub service_types: SerializableSlab<GqlType>,

    pub contexts: MappedArena<ContextType>,
    pub context_types: SerializableSlab<GqlType>,

    // query related
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub database_queries: MappedArena<Query>,
    pub service_queries: MappedArena<Query>,

    // mutation related
    pub mutation_types: SerializableSlab<GqlType>, // create, update, delete input types such as `PersonUpdateInput`
    pub database_mutations: MappedArena<Mutation>,
    pub service_mutations: MappedArena<Mutation>,

    // service related
    pub methods: SerializableSlab<ServiceMethod>,
    pub scripts: SerializableSlab<Script>,

    pub tables: SerializableSlab<PhysicalTable>,
}

impl Default for ModelSystem {
    fn default() -> Self {
        ModelSystem {
            primitive_types: SerializableSlab::new(),
            database_types: SerializableSlab::new(),
            service_types: SerializableSlab::new(),
            contexts: MappedArena::default(),
            context_types: SerializableSlab::new(),
            order_by_types: SerializableSlab::new(),
            predicate_types: SerializableSlab::new(),
            database_queries: MappedArena::default(),
            service_queries: MappedArena::default(),
            mutation_types: SerializableSlab::new(),
            database_mutations: MappedArena::default(),
            service_mutations: MappedArena::default(),
            tables: SerializableSlab::new(),
            methods: SerializableSlab::new(),
            scripts: SerializableSlab::new(),
        }
    }
}
