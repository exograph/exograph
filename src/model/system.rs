use id_arena::Arena;

use super::{operation::*, predicate_builder, system_context::MappedArena};
use super::{order::*, type_builder};
use super::{order_by_type_builder, predicate::*};

use crate::{model::ast::ast_types::*, sql::table::PhysicalTable};
use crate::{
    model::{query_builder, system_context::SystemContextBuilding},
    sql::database::Database,
};

use super::types::ModelType;

#[derive(Debug, Clone)]
pub struct ModelSystem {
    pub types: Arena<ModelType>,
    pub order_by_types: Arena<OrderByParameterType>,
    pub predicate_types: Arena<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub tables: Arena<PhysicalTable>,
    pub database: Database,
}

impl ModelSystem {
    pub fn build(ast_types: &[AstType]) -> ModelSystem {
        let mut building = SystemContextBuilding::new();
        type_builder::build_shallow(ast_types, &mut building);
        query_builder::build_shallow(ast_types, &mut building);
        order_by_type_builder::build_shallow(ast_types, &mut building);
        predicate_builder::build_shallow(ast_types, &mut building);

        type_builder::build_expanded(ast_types, &mut building);
        order_by_type_builder::build_expanded(&mut building);
        predicate_builder::build_expanded(&mut building);
        query_builder::build_expanded(&mut building);

        ModelSystem {
            types: building.types.values,
            order_by_types: building.order_by_types.values,
            predicate_types: building.predicate_types.values,
            queries: building.queries,
            tables: building.tables.values,

            database: Database::empty(),
        }
    }
}
