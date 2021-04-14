use id_arena::Id;

use super::{
    operation::*, predicate_builder, system_context::MappedArena, types::ModelTypeModifier,
};
use super::{order::*, type_builder};
use super::{order_by_type_builder, predicate::*};

use crate::{model::ast::ast_types::*, sql::table::PhysicalTable};
use crate::{model::system_context::SystemContextBuilding, sql::database::Database};

use super::types::ModelType;

#[derive(Debug, Clone)]
pub struct ModelSystem {
    pub types: MappedArena<ModelType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,
    pub queries: Vec<Query>,
    pub tables: MappedArena<PhysicalTable>,
    pub database: Database,
}

impl ModelSystem {
    pub fn build(ast_types: &[AstType]) -> ModelSystem {
        let mut building = SystemContextBuilding::new();
        type_builder::build(ast_types, &mut building);
        order_by_type_builder::build(&mut building);
        predicate_builder::build(&mut building);

        let queries: Vec<Query> = building
            .types
            .values
            .iter()
            .flat_map(|tpe| tpe.1.queries(&building))
            .collect();

        ModelSystem {
            types: building.types,
            order_by_types: building.order_by_types,
            predicate_types: building.predicate_types,
            queries: queries,
            tables: building.tables,

            database: Database::empty(),
        }
    }

    pub fn pk_query(&self, model_type_id: &Id<ModelType>) -> &Query {
        self.queries
            .iter()
            .find(|query| {
                query.return_type.type_id == *model_type_id
                    && query.return_type.type_modifier == ModelTypeModifier::NonNull
            })
            .unwrap()
    }
}
