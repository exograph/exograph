// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::{
    Positioned,
    types::{InputObjectType, InputValueDefinition, TypeDefinition, TypeKind},
};
use core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        Parameter, Type, TypeDefinitionIntrospection, TypeDefinitionProvider,
        default_positioned_name,
    },
    types::{FieldType, Named, TypeValidation},
};
use serde::{Deserialize, Serialize};

use super::types::ModuleType;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameter {
    pub name: String,
    pub typ: FieldType<ArgumentParameterType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameterType {
    pub name: String,
    pub type_id: Option<SerializableSlabIndex<ModuleType>>,
    pub is_primitive: bool,
}

impl Named for ArgumentParameterType {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Parameter for ArgumentParameter {
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

impl TypeDefinitionProvider<SerializableSlab<ModuleType>> for ArgumentParameterType {
    fn type_definition(&self, module_types: &SerializableSlab<ModuleType>) -> TypeDefinition {
        let type_def = module_types
            .get(self.type_id.unwrap())
            .unwrap()
            .type_definition(module_types);

        let kind = match type_def.fields() {
            Some(fields) => TypeKind::InputObject(InputObjectType {
                fields: fields
                    .iter()
                    .map(|positioned| {
                        let field_definition = &positioned.node;

                        Positioned {
                            pos: positioned.pos,
                            node: InputValueDefinition {
                                description: field_definition.description.clone(),
                                name: field_definition.name.clone(),
                                ty: field_definition.ty.clone(),
                                default_value: None,
                                directives: vec![],
                            },
                        }
                    })
                    .collect(),
            }),
            None => TypeKind::Scalar,
        };

        TypeDefinition {
            extend: false,
            name: default_positioned_name(&self.name),
            description: None,
            directives: vec![],
            kind,
        }
    }
}

impl TypeDefinitionProvider<SerializableSlab<ModuleType>> for ArgumentParameter {
    fn type_definition(&self, _system: &SerializableSlab<ModuleType>) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}
