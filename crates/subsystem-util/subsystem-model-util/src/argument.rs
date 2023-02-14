use async_graphql_parser::{
    types::{InputObjectType, InputValueDefinition, Type, TypeDefinition, TypeKind},
    Positioned,
};
use core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        default_positioned_name, Parameter, TypeDefinitionIntrospection, TypeDefinitionProvider,
    },
    types::{FieldType, Named},
};
use serde::{Deserialize, Serialize};

use super::types::ServiceType;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameter {
    pub name: String,
    pub typ: FieldType<ArgumentParameterType>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameterType {
    pub name: String,
    pub type_id: Option<SerializableSlabIndex<ServiceType>>,
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
}

impl TypeDefinitionProvider<SerializableSlab<ServiceType>> for ArgumentParameterType {
    fn type_definition(&self, service_types: &SerializableSlab<ServiceType>) -> TypeDefinition {
        let type_def = service_types
            .get(self.type_id.unwrap())
            .unwrap()
            .type_definition(service_types);

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

impl TypeDefinitionProvider<SerializableSlab<ServiceType>> for ArgumentParameter {
    fn type_definition(&self, _system: &SerializableSlab<ServiceType>) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}
