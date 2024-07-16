// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::types::{
    FieldDefinition, InputObjectType, ObjectType, Type, TypeDefinition, TypeKind,
};

use core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
        Parameter, TypeDefinitionProvider,
    },
    types::{FieldType, Named, OperationReturnType},
};

use serde::{Deserialize, Serialize};

use crate::access::Access;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleType {
    pub name: String,
    pub kind: ModuleTypeKind,
    pub is_input: bool, // Is this to be used as an input field (such as an argument in a mutation)? Needed for introspection
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ModuleTypeKind {
    Primitive,
    Composite(ModuleCompositeType),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleCompositeType {
    pub fields: Vec<ModuleField>,
    pub is_input: bool,
    pub access: Access,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleField {
    pub name: String,
    pub typ: FieldType<ModuleFieldType>,
    pub has_default_value: bool, // does this field have a default value?
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModuleFieldType {
    pub type_id: SerializableSlabIndex<ModuleType>,
    pub type_name: String,
}

impl Named for ModuleFieldType {
    fn name(&self) -> &str {
        &self.type_name
    }
}

impl TypeDefinitionProvider<SerializableSlab<ModuleType>> for ModuleType {
    fn type_definition(&self, module_types: &SerializableSlab<ModuleType>) -> TypeDefinition {
        match &self.kind {
            ModuleTypeKind::Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
            ModuleTypeKind::Composite(ModuleCompositeType {
                fields: model_fields,
                ..
            }) => {
                let kind = if self.is_input {
                    let fields = model_fields
                        .iter()
                        .map(|model_field| default_positioned(model_field.input_value()))
                        .collect();
                    TypeKind::InputObject(InputObjectType { fields })
                } else {
                    let fields: Vec<_> = model_fields
                        .iter()
                        .map(|model_field| {
                            default_positioned(model_field.field_definition(module_types))
                        })
                        .collect();

                    TypeKind::Object(ObjectType {
                        implements: vec![],
                        fields,
                    })
                };
                TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(&self.name),
                    directives: vec![],
                    kind,
                }
            }
        }
    }
}

impl FieldDefinitionProvider<SerializableSlab<ModuleType>> for ModuleField {
    fn field_definition(&self, _module_types: &SerializableSlab<ModuleType>) -> FieldDefinition {
        let field_type = default_positioned((&self.typ).into());

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments: vec![],
            ty: field_type,
            directives: vec![],
        }
    }
}

impl Parameter for ModuleField {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ModuleOperationReturnType {
    Own(OperationReturnType<ModuleType>),
    Foreign(FieldType<ForeignModuleType>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ForeignModuleType {
    pub module_name: String,
    pub return_type_name: String,
}

impl Named for ForeignModuleType {
    fn name(&self) -> &str {
        &self.return_type_name
    }
}
