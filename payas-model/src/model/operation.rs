use id_arena::Id;
use payas_sql::sql::PhysicalTable;

use crate::model::{GqlCompositeTypeKind, GqlTypeKind};

use super::{
    order::OrderByParameter,
    predicate::PredicateParameter,
    system::ModelSystem,
    types::{GqlType, GqlTypeModifier},
};

#[derive(Debug, Clone)]
pub struct Query {
    pub name: String,
    pub predicate_param: Option<PredicateParameter>,
    pub order_by_param: Option<OrderByParameter>,
    pub return_type: OperationReturnType,
}

#[derive(Debug, Clone)]
pub struct Mutation {
    pub name: String,
    pub kind: MutationKind,
    pub return_type: OperationReturnType,
}

#[derive(Debug, Clone)]
pub enum MutationKind {
    Create(MutationDataParameter),
    Delete(PredicateParameter),
    Update {
        data_param: MutationDataParameter,
        predicate_param: PredicateParameter,
    },
}

#[derive(Debug, Clone)]
pub struct MutationDataParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: Id<GqlType>,
}

#[derive(Debug, Clone)]
pub struct OperationReturnType {
    pub type_id: Id<GqlType>,
    pub type_name: String,
    pub type_modifier: GqlTypeModifier,
}

impl OperationReturnType {
    pub fn typ<'a>(&self, system: &'a ModelSystem) -> &'a GqlType {
        let return_type_id = &self.type_id;
        &system.types[*return_type_id]
    }

    pub fn physical_table<'a>(&self, system: &'a ModelSystem) -> &'a PhysicalTable {
        let return_type = self.typ(system);
        match &return_type.kind {
            GqlTypeKind::Primitive => panic!(),
            GqlTypeKind::Composite(GqlCompositeTypeKind {
                fields: _,
                table_id,
                ..
            }) => &system.tables[*table_id],
        }
    }
}
