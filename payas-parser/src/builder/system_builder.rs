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
use crate::builder::typechecking::{Scope, Typecheck};

use super::{
    mutation_builder, order_by_type_builder, predicate_builder, query_builder, type_builder,
    typechecking::Type,
};

pub fn build(ast_system: AstSystem) -> ModelSystem {
    let mut building = SystemContextBuilding::default();

    let ast_types = &ast_system.models;

    let mut types_arena: MappedArena<Type> = MappedArena::default();
    for model in ast_types {
        types_arena.add(model.name.as_str(), model.shallow());
    }

    loop {
        let mut did_change = false;
        let init_scope = Scope {
            enclosing_model: None,
        };
        for model in ast_types {
            let mut typ = types_arena
                .get_by_key_mut(model.name.as_str())
                .unwrap()
                .clone();
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

    let mut types_types = Vec::new();

    for ast_type in types_arena.keys() {
        types_types.push(types_arena.get_by_key(ast_type).unwrap().clone());
    }

    type_builder::build_shallow(&types_arena, &mut building);

    query_builder::build_shallow(&types_types, &mut building);
    order_by_type_builder::build_shallow(&types_types, &mut building);
    predicate_builder::build_shallow(&types_types, &mut building);

    type_builder::build_expanded(&types_arena, &mut building);
    order_by_type_builder::build_expanded(&mut building);
    predicate_builder::build_expanded(&mut building);
    query_builder::build_expanded(&mut building);

    mutation_builder::build(&types_types, &mut building);

    ModelSystem {
        types: building.types.values,
        order_by_types: building.order_by_types.values,
        predicate_types: building.predicate_types.values,
        queries: building.queries,
        tables: building.tables.values,
        mutation_types: building.mutation_types.values,
        create_mutations: building.mutations,
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
