use std::collections::HashMap;

use id_arena::Arena;
use payas_model::{
    model::{
        mapped_arena::MappedArena,
        operation::{Mutation, Query},
        order::OrderByParameterType,
        predicate::PredicateParameterType,
        system::ModelSystem,
        types::ModelType,
    },
    sql::PhysicalTable,
};

use crate::ast::ast_types::AstSystem;
use crate::builder::typechecking::{Typecheck, Scope};

use super::{mutation_builder, order_by_type_builder, predicate_builder, query_builder, type_builder, typechecking::Type};

pub fn build(ast_system: AstSystem) -> ModelSystem {
    // let mut building = SystemContextBuilding::default();

    let ast_types = &ast_system.models;
    let mut ast_types_map = HashMap::new();
    for ast_type in ast_types {
        ast_types_map.insert(ast_type.name.clone(), ast_type);
    }

    let mut types_arena: MappedArena<Type> = MappedArena::default();
    for model in ast_types {
        types_arena.add(model.name.as_str(), model.shallow());
    }

    loop {
        let mut did_change = false;
        let init_scope = Scope { enclosing_model: None };
        for model in ast_types {
            let mut typ = types_arena.get_by_key_mut(model.name.as_str()).unwrap().clone();
            let pass_res = model.pass(&mut typ, &types_arena, &init_scope);
            if pass_res {
                *types_arena.get_by_key_mut(model.name.as_str()).unwrap() = typ;
                did_change = true;
            }
        }

        if !did_change {
            break;
        }
    }

    dbg!(types_arena);

    // type_builder::build_shallow(&ast_types_map, &mut building);

    // query_builder::build_shallow(&ast_types, &mut building);
    // order_by_type_builder::build_shallow(&ast_types, &mut building);
    // predicate_builder::build_shallow(&ast_types, &mut building);

    // type_builder::build_expanded(&ast_types_map, &mut building);
    // order_by_type_builder::build_expanded(&mut building);
    // predicate_builder::build_expanded(&mut building);
    // query_builder::build_expanded(&mut building);

    // mutation_builder::build(&ast_types, &mut building);

    ModelSystem {
        types: Default::default(),
        order_by_types: Default::default(),
        predicate_types: Default::default(),
        queries: Default::default(),
        tables: Default::default(),
        mutation_types: Default::default(),
        create_mutations: Default::default(),
    }
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub types: MappedArena<ModelType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: MappedArena<ModelType>,
    pub mutations: MappedArena<Mutation>,
    pub tables: MappedArena<PhysicalTable>,
}
