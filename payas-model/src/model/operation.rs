use std::fmt::Debug;

use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

use crate::model::{GqlCompositeType, GqlCompositeTypeKind, GqlTypeKind};

use super::{
    argument::ArgumentParameter,
    interceptor::{Interceptor, InterceptorKind},
    limit_offset::{LimitParameter, OffsetParameter},
    mapped_arena::SerializableSlabIndex,
    order::OrderByParameter,
    predicate::PredicateParameter,
    service::ServiceMethod,
    system::ModelSystem,
    types::{GqlType, GqlTypeModifier},
};

pub trait GraphQLOperation: Debug {
    fn name(&self) -> &str;

    fn interceptors(&self) -> &Interceptors;

    fn return_type(&self) -> &OperationReturnType;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Query {
    pub name: String,
    pub kind: QueryKind,
    pub return_type: OperationReturnType,
    pub interceptors: Interceptors,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum QueryKind {
    Database(Box<DatabaseQueryParameter>),
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

impl GraphQLOperation for Query {
    fn name(&self) -> &str {
        &self.name
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn return_type(&self) -> &OperationReturnType {
        &self.return_type
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mutation {
    pub name: String,
    pub kind: MutationKind,
    pub return_type: OperationReturnType,
    pub interceptors: Interceptors,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MutationKind {
    // mutations for persistent queries
    Database {
        kind: DatabaseMutationKind,
    },

    // mutation for service
    Service {
        method_id: Option<SerializableSlabIndex<ServiceMethod>>,
        argument_param: Vec<ArgumentParameter>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DatabaseMutationKind {
    Create(CreateDataParameter),
    Delete(PredicateParameter),
    Update {
        data_param: UpdateDataParameter,
        predicate_param: PredicateParameter,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Interceptors {
    pub interceptors: Vec<Interceptor>,
}

impl Interceptors {
    pub fn ordered(&self) -> Vec<&Interceptor> {
        let mut processed = Vec::new();
        let mut deferred = Vec::new();

        for interceptor in &self.interceptors {
            if interceptor.interceptor_kind == InterceptorKind::Before {
                processed.push(interceptor);
            } else {
                deferred.push(interceptor);
            }
        }
        processed.extend(deferred.into_iter());
        processed
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDataParameter {
    pub name: String,
    pub typ: CreateDataParameterTypeWithModifier,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateDataParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDataParameterTypeWithModifier {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<GqlType>,
    pub array_input: bool, // does it take an array parameter? For create<Entity>s (note the plural), this is set to true
}

impl GraphQLOperation for Mutation {
    fn name(&self) -> &str {
        &self.name
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn return_type(&self) -> &OperationReturnType {
        &self.return_type
    }
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
