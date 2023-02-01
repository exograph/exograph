use std::fmt::Debug;

use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
use core_plugin_interface::core_model::type_normalization::{Operation, Parameter, TypeModifier};
use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

use crate::{model::ModelPostgresSystem, types::PostgresCompositeType};

use super::{
    limit_offset::{LimitParameter, OffsetParameter},
    order::OrderByParameter,
    predicate::PredicateParameter,
    types::PostgresTypeModifier,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Query<P>
where
    P: OperationParameter,
{
    pub name: String,
    pub parameter: P,
    pub return_type: OperationReturnType,
}

pub trait OperationParameter {
    fn parameters(&self) -> Vec<&dyn Parameter>;
}

pub type PkQuery = Query<PkQueryParameter>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PkQueryParameter {
    pub predicate_param: PredicateParameter,
}

impl OperationParameter for PkQueryParameter {
    fn parameters(&self) -> Vec<&dyn Parameter> {
        vec![&self.predicate_param]
    }
}

pub type CollectionQuery = Query<CollectionQueryParameter>;

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AggregateQueryParameter {
    pub predicate_param: PredicateParameter,
}

impl OperationParameter for AggregateQueryParameter {
    fn parameters(&self) -> Vec<&dyn Parameter> {
        vec![&self.predicate_param]
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresMutation {
    pub name: String,
    pub kind: PostgresMutationKind,
    pub return_type: OperationReturnType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    pub typ: CreateDataParameterTypeWithModifier,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateDataParameter {
    pub name: String,
    pub type_name: String,
    pub type_id: SerializableSlabIndex<PostgresCompositeType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDataParameterTypeWithModifier {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<PostgresCompositeType>,
    pub array_input: bool, // does it take an array parameter? For create<Entity>s (note the plural), this is set to true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationReturnType {
    pub type_id: SerializableSlabIndex<PostgresCompositeType>,
    pub type_name: String,
    pub type_modifier: PostgresTypeModifier,
}

impl OperationReturnType {
    pub fn typ<'a>(&'a self, system: &'a ModelPostgresSystem) -> &PostgresCompositeType {
        &system.entity_types[self.type_id]
    }

    pub fn physical_table<'a>(&self, system: &'a ModelPostgresSystem) -> &'a PhysicalTable {
        let return_type = self.typ(system);
        &system.tables[return_type.table_id]
    }
}

impl Parameter for CreateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.typ.type_name
    }

    fn type_modifier(&self) -> TypeModifier {
        if self.typ.array_input {
            TypeModifier::List
        } else {
            TypeModifier::NonNull
        }
    }
}

impl Parameter for UpdateDataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> TypeModifier {
        TypeModifier::NonNull
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

    fn return_type_name(&self) -> &str {
        &self.return_type.type_name
    }

    fn return_type_modifier(&self) -> TypeModifier {
        (&self.return_type.type_modifier).into()
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

    fn return_type_name(&self) -> &str {
        &self.return_type.type_name
    }

    fn return_type_modifier(&self) -> TypeModifier {
        (&self.return_type.type_modifier).into()
    }
}
