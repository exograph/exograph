use std::fmt::Debug;

use async_graphql_parser::types::Type;
use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
use core_plugin_interface::core_model::types::OperationReturnType;
use core_plugin_interface::core_model::types::{FieldType, Named};

use core_plugin_interface::core_model::type_normalization::{Operation, Parameter};
use serde::{Deserialize, Serialize};

use crate::types::EntityType;
use crate::types::MutationType;

use super::{
    limit_offset::{LimitParameter, OffsetParameter},
    order::OrderByParameter,
    predicate::PredicateParameter,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct Query<P>
where
    P: OperationParameter,
{
    pub name: String,
    pub parameter: P,
    pub return_type: OperationReturnType<EntityType>,
}

pub trait OperationParameter {
    fn parameters(&self) -> Vec<&dyn Parameter>;
}

pub type PkQuery = Query<PkQueryParameter>;

#[derive(Serialize, Deserialize, Debug)]
pub struct PkQueryParameter {
    pub predicate_param: PredicateParameter,
}

impl OperationParameter for PkQueryParameter {
    fn parameters(&self) -> Vec<&dyn Parameter> {
        vec![&self.predicate_param]
    }
}

pub type CollectionQuery = Query<CollectionQueryParameter>;

#[derive(Serialize, Deserialize, Debug)]
pub struct CollectionQueryParameter {
    pub predicate_param: PredicateParameter,
    pub order_by_param: OrderByParameter,
    pub limit_param: LimitParameter,
    pub offset_param: OffsetParameter,
}

impl OperationParameter for CollectionQueryParameter {
    fn parameters(&self) -> Vec<&dyn Parameter> {
        vec![
            &self.predicate_param,
            &self.order_by_param,
            &self.limit_param,
            &self.offset_param,
        ]
    }
}

pub type AggregateQuery = Query<AggregateQueryParameter>;

#[derive(Serialize, Deserialize, Debug)]
pub struct AggregateQueryParameter {
    pub predicate_param: PredicateParameter,
}

impl OperationParameter for AggregateQueryParameter {
    fn parameters(&self) -> Vec<&dyn Parameter> {
        vec![&self.predicate_param]
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresMutation {
    pub name: String,
    pub kind: PostgresMutationKind,
    pub return_type: OperationReturnType<EntityType>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PostgresMutationKind {
    Create(CreateDataParameter),
    Delete(PredicateParameter),
    Update {
        data_param: UpdateDataParameter,
        predicate_param: PredicateParameter,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDataParameter {
    pub name: String,
    // FieldType will be list for array input such as for create<Entity>s (note the plural)
    pub typ: FieldType<CreateDataParameterType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateDataParameter {
    pub name: String,
    pub type_name: String,
    pub typ: FieldType<UpdateDataParameterType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateDataParameterType {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<MutationType>,
}

impl Named for UpdateDataParameterType {
    fn name(&self) -> &str {
        &self.type_name
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDataParameterType {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<MutationType>,
}

impl Named for CreateDataParameterType {
    fn name(&self) -> &str {
        &self.type_name
    }
}

impl Parameter for CreateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }
}

impl Parameter for UpdateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }
}

impl<P> Operation for Query<P>
where
    P: OperationParameter,
{
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        self.parameter.parameters()
    }

    fn return_type(&self) -> Type {
        (&self.return_type).into()
    }
}

impl Operation for PostgresMutation {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        match &self.kind {
            PostgresMutationKind::Create(data_param) => vec![data_param],
            PostgresMutationKind::Delete(predicate_param) => vec![predicate_param],
            PostgresMutationKind::Update {
                data_param,
                predicate_param,
            } => vec![predicate_param, data_param],
        }
    }
    fn return_type(&self) -> Type {
        (&self.return_type).into()
    }
}
