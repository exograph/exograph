use async_graphql_parser::types::TypeDefinition;
use async_graphql_parser::types::TypeKind;
use payas_core_model::type_normalization::default_positioned_name;
use payas_core_model::type_normalization::Parameter;
use payas_core_model::type_normalization::TypeDefinitionProvider;
use payas_core_model::type_normalization::TypeModifier;
use serde::{Deserialize, Serialize};

use crate::model::ModelDatabaseSystem;

use super::types::DatabaseType;

use super::types::DatabaseTypeModifier;
use payas_core_model::mapped_arena::SerializableSlabIndex;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LimitParameter {
    pub name: String,
    pub typ: LimitParameterType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LimitParameterType {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<DatabaseType>,
    pub type_modifier: DatabaseTypeModifier,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OffsetParameter {
    pub name: String,
    pub typ: OffsetParameterType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OffsetParameterType {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<DatabaseType>,
    pub type_modifier: DatabaseTypeModifier,
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

impl TypeDefinitionProvider<ModelDatabaseSystem> for LimitParameter {
    fn type_definition(&self, _system: &ModelDatabaseSystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider<ModelDatabaseSystem> for OffsetParameter {
    fn type_definition(&self, _system: &ModelDatabaseSystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}
