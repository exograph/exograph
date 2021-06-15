use id_arena::Arena;

use super::order::*;
use super::predicate::*;
use super::ContextType;
use super::{mapped_arena::MappedArena, operation::*};

use crate::sql::PhysicalTable;

use super::types::GqlType;

#[derive(Debug, Clone, Default)]
pub struct ModelSystem {
    pub types: Arena<GqlType>,
    pub contexts: Arena<ContextType>,
    pub order_by_types: Arena<OrderByParameterType>,
    pub predicate_types: Arena<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: Arena<GqlType>,
    pub create_mutations: MappedArena<Mutation>,
    pub tables: Arena<PhysicalTable>,
}
