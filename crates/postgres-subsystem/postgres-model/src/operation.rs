use std::fmt::Debug;

use core_plugin_interface::core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{Operation, Parameter, TypeModifier},
};
use payas_sql::PhysicalTable;
use serde::{Deserialize, Serialize};

use crate::{
    model::ModelPostgresSystem,
    types::{PostgresCompositeType, PostgresTypeKind},
};

use super::{
    limit_offset::{LimitParameter, OffsetParameter},
    order::OrderByParameter,
    predicate::PredicateParameter,
    types::{PostgresType, PostgresTypeModifier},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresQuery {
    pub name: String,
    pub parameter: PostgresQueryParameter,
    pub return_type: OperationReturnType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresQueryParameter {
    pub predicate_param: Option<PredicateParameter>,
    pub order_by_param: Option<OrderByParameter>,
    pub limit_param: Option<LimitParameter>,
    pub offset_param: Option<OffsetParameter>,
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
    pub type_id: SerializableSlabIndex<PostgresType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateDataParameterTypeWithModifier {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<PostgresType>,
    pub array_input: bool, // does it take an array parameter? For create<Entity>s (note the plural), this is set to true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationReturnType {
    pub type_id: SerializableSlabIndex<PostgresType>,
    pub type_name: String,
    pub type_modifier: PostgresTypeModifier,
}

impl OperationReturnType {
    pub fn typ<'a>(&self, system: &'a ModelPostgresSystem) -> &'a PostgresType {
        &system.postgres_types[self.type_id]
    }

    pub fn physical_table<'a>(&self, system: &'a ModelPostgresSystem) -> &'a PhysicalTable {
        let return_type = self.typ(system);
        match &return_type.kind {
            PostgresTypeKind::Primitive => panic!(),
            PostgresTypeKind::Composite(PostgresCompositeType {
                fields: _,
                table_id,
                ..
            }) => &system.tables[*table_id],
        }
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

impl Operation for PostgresQuery {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        let mut params: Vec<&dyn Parameter> = vec![];

        macro_rules! populate_params (
            ($param_name:expr) => {
                match $param_name {
                    Some(param) => params.push(param),
                    None => {}
                }
            }
        );

        let PostgresQueryParameter {
            predicate_param,
            order_by_param,
            limit_param,
            offset_param,
        } = &self.parameter;
        populate_params!(&predicate_param);
        populate_params!(&order_by_param);
        populate_params!(&limit_param);
        populate_params!(&offset_param);

        params
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
