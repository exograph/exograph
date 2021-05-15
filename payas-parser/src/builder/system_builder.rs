use std::collections::HashMap;

use payas_model::{
    model::{
        mapped_arena::MappedArena,
        operation::{Mutation, Query},
        order::OrderByParameterType,
        predicate::PredicateParameterType,
        system::ModelSystem,
        types::ModelType,
    },
    sql::{database::Database, PhysicalTable},
};

use crate::ast::ast_types::AstSystem;

use super::{
    mutation_builder, order_by_type_builder, predicate_builder, query_builder, type_builder,
};

pub fn build(ast_system: AstSystem) -> ModelSystem {
    let mut building = SystemContextBuilding::new();

    let ast_types = &ast_system.types;
    let mut ast_types_map = HashMap::new();
    for ast_type in ast_types {
        ast_types_map.insert(ast_type.name.clone(), ast_type);
    }

    type_builder::build_shallow(&ast_types_map, &mut building);

    query_builder::build_shallow(&ast_types, &mut building);
    order_by_type_builder::build_shallow(&ast_types, &mut building);
    predicate_builder::build_shallow(&ast_types, &mut building);

    type_builder::build_expanded(&ast_types_map, &mut building);
    order_by_type_builder::build_expanded(&mut building);
    predicate_builder::build_expanded(&mut building);
    query_builder::build_expanded(&mut building);

    mutation_builder::build(&ast_types, &mut building);

    ModelSystem {
        types: building.types.values,
        order_by_types: building.order_by_types.values,
        predicate_types: building.predicate_types.values,
        queries: building.queries,
        tables: building.tables.values,
        mutation_types: building.mutation_types.values,
        create_mutations: building.mutations,

        database: Database::from_env(),
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
