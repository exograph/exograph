use payas_sql::sql::PhysicalTable;
use serde::{Deserialize, Serialize};

use crate::model::{GqlCompositeType, GqlCompositeTypeKind, GqlTypeKind};

use super::{
    argument::ArgumentParameter,
    limit_offset::{LimitParameter, OffsetParameter},
    mapped_arena::SerializableSlabIndex,
    order::OrderByParameter,
    predicate::PredicateParameter,
    service::ServiceMethod,
    system::ModelSystem,
    types::{GqlType, GqlTypeModifier},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Query {
    pub name: String,
    pub kind: QueryKind,
    pub return_type: OperationReturnType,
    pub intercetors: Interceptors,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum QueryKind {
    Database(DatabaseQueryParameter),
    Service {
        method_id: Option<SerializableSlabIndex<ServiceMethod>>,
        argument_param: Vec<ArgumentParameter>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatabaseQueryParameter {
    pub predicate_param: Option<PredicateParameter>,
    pub order_by_param: Option<OrderByParameter>,
    pub limit_param: Option<LimitParameter>,
    pub offset_param: Option<OffsetParameter>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mutation {
    pub name: String,
    pub kind: MutationKind,
    pub return_type: OperationReturnType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MutationKind {
    // mutations for persistent queries
    Create(CreateDataParameter),
    Delete(PredicateParameter),
    Update {
        data_param: UpdateDataParameter,
        predicate_param: PredicateParameter,
    },

    // mutation for service
    Service {
        method_id: Option<SerializableSlabIndex<ServiceMethod>>,
        argument_param: Vec<ArgumentParameter>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Interceptors {
    pub interceptors: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDataParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
    pub array_input: bool, // does it take an array parameter? For create<Entity>s (note the plural), this is set to true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateDataParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationReturnType {
    pub type_id: SerializableSlabIndex<GqlType>,
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
            GqlTypeKind::Composite(GqlCompositeType {
                fields: _,
                kind: GqlCompositeTypeKind::Persistent { table_id, .. },
                ..
            }) => &system.tables[*table_id],
            _ => panic!(),
        }
    }
}
