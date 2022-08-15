use super::argument::ArgumentParameterType;
use super::column_id::ColumnId;
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
use payas_sql::Column;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSystem {
    pub types: SerializableSlab<GqlType>,
    pub contexts: MappedArena<ContextType>,
    pub argument_types: SerializableSlab<ArgumentParameterType>,
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: SerializableSlab<GqlType>,
    pub mutations: MappedArena<Mutation>,
    pub tables: SerializableSlab<PhysicalTable>,
    pub methods: SerializableSlab<ServiceMethod>,
    pub scripts: SerializableSlab<Script>,
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

impl ModelSystem {
    pub fn create_column_with_id<'a>(&'a self, column_id: &ColumnId) -> Column<'a> {
        Column::Physical(column_id.get_column(self))
    }

    pub fn query_by_name(&self, name: &str) -> Option<&Query> {
        self.queries.get_by_key(name)
    }

    pub fn mutation_by_name(&self, name: &str) -> Option<&Mutation> {
        self.mutations.get_by_key(name)
    }
}
