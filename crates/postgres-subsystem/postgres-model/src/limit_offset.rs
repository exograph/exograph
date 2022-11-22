use super::types::{PostgresType, PostgresTypeModifier};
use crate::model::ModelPostgresSystem;
use async_graphql_parser::types::{TypeDefinition, TypeKind};
use core_plugin_interface::core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{
        default_positioned_name, Parameter, TypeDefinitionProvider, TypeModifier,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LimitParameter {
    pub name: String,
    pub typ: LimitParameterType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LimitParameterType {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<PostgresType>,
    pub type_modifier: PostgresTypeModifier,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OffsetParameter {
    pub name: String,
    pub typ: OffsetParameterType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OffsetParameterType {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<PostgresType>,
    pub type_modifier: PostgresTypeModifier,
}

impl Parameter for LimitParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.typ.type_name
    }

    fn type_modifier(&self) -> TypeModifier {
        (&self.typ.type_modifier).into()
    }
}

impl Parameter for OffsetParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.typ.type_name
    }

    fn type_modifier(&self) -> TypeModifier {
        (&self.typ.type_modifier).into()
    }
}

impl TypeDefinitionProvider<ModelPostgresSystem> for LimitParameter {
    fn type_definition(&self, _system: &ModelPostgresSystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider<ModelPostgresSystem> for OffsetParameter {
    fn type_definition(&self, _system: &ModelPostgresSystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}
