use codemap::CodeMap;
use payas_model::{
    model::{
        mapped_arena::MappedArena,
        operation::{Mutation, Query},
        order::OrderByParameterType,
        predicate::PredicateParameterType,
        system::ModelSystem,
        types::GqlType,
        ContextType,
    },
    sql::PhysicalTable,
};

use crate::ast::ast_types::AstSystem;

use super::{
    context_builder, mutation_builder, order_by_type_builder, predicate_builder, query_builder,
    resolved_builder, type_builder,
};

use crate::typechecker;

pub fn build(ast_system: AstSystem, codemap: CodeMap) -> ModelSystem {
    let resolved_system = resolved_builder::build(typechecker::build(ast_system, codemap));
    let resolved_types = resolved_system.types;
    let resolved_contexts = resolved_system.contexts;

    let mut building = SystemContextBuilding::default();

    // First build shallow GQL types for model, queries, query parameters
    type_builder::build_shallow(&resolved_types, &mut building);
    context_builder::build_shallow(&resolved_contexts, &mut building);

    // The next set of shallow builders need GQL types build above (the order of the next three is unimportant)
    order_by_type_builder::build_shallow(&resolved_types, &mut building);
    predicate_builder::build_shallow(&resolved_types, &mut building);
    query_builder::build_shallow(&resolved_types, &mut building);

    // Now expand the types
    // First fully build the model types
    type_builder::build_expanded(&resolved_types, &mut building);
    context_builder::build_expanded(&resolved_contexts, &mut building);

    // Which is then used to expand query and query parameters (the order of the next three is unimportant)
    query_builder::build_expanded(&mut building);
    order_by_type_builder::build_expanded(&mut building);
    predicate_builder::build_expanded(&mut building);

    // Finally build mutations. We don't need a shallow pass, since all the types (predicates, specifically) have been already built
    mutation_builder::build(&resolved_types, &mut building);

    ModelSystem {
        types: building.types.values,
        contexts: building.contexts.values,
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
    pub types: MappedArena<GqlType>,
    pub contexts: MappedArena<ContextType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: MappedArena<GqlType>,
    pub mutations: MappedArena<Mutation>,
    pub tables: MappedArena<PhysicalTable>,
}
