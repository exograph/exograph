// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::subsystem::PostgresGraphQLSubsystem;
use async_graphql_parser::types::{TypeDefinition, TypeKind};
use core_plugin_interface::core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{default_positioned_name, Parameter, Type, TypeDefinitionProvider},
    types::{FieldType, Named, TypeValidation},
};
use serde::{Deserialize, Serialize};

use postgres_core_model::types::PostgresPrimitiveType;

#[derive(Serialize, Deserialize, Debug)]
pub struct LimitParameter {
    pub name: String,
    pub typ: FieldType<LimitParameterType>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LimitParameterType {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<PostgresPrimitiveType>,
}

impl Named for LimitParameterType {
    fn name(&self) -> &str {
        &self.type_name
    }
}

impl Parameter for LimitParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }

    fn type_validation(&self) -> Option<TypeValidation> {
        None
    }
}

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for LimitParameter {
    fn type_definition(&self, _system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OffsetParameter {
    pub name: String,
    pub typ: FieldType<OffsetParameterType>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OffsetParameterType {
    pub type_name: String,
    pub type_id: SerializableSlabIndex<PostgresPrimitiveType>,
}

impl Named for OffsetParameterType {
    fn name(&self) -> &str {
        &self.type_name
    }
}

impl Parameter for OffsetParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }

    fn type_validation(&self) -> Option<TypeValidation> {
        None
    }
}

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for OffsetParameter {
    fn type_definition(&self, _system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}
