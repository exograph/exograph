use id_arena::Arena;

use super::order::*;
use super::predicate::*;
use super::{mapped_arena::MappedArena, operation::*};

use crate::sql::database::Database;
use crate::sql::PhysicalTable;

use super::types::ModelType;

#[derive(Debug, Clone)]
pub struct ModelSystem {
    pub types: Arena<ModelType>,
    pub order_by_types: Arena<OrderByParameterType>,
    pub predicate_types: Arena<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: Arena<ModelType>,
    pub create_mutations: MappedArena<Mutation>,
    pub tables: Arena<PhysicalTable>,
    pub database: Database,
}
