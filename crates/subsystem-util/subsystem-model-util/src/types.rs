use async_graphql_parser::types::{
    FieldDefinition, InputObjectType, ObjectType, Type, TypeDefinition, TypeKind,
};

use core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
        Parameter, TypeDefinitionProvider,
    },
    types::{FieldType, Named},
};

use serde::{Deserialize, Serialize};

use crate::access::Access;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceType {
    pub name: String,
    pub kind: ServiceTypeKind,
    pub is_input: bool, // Is this to be used as an input field (such as an argument in a mutation)? Needed for introspection
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ServiceTypeKind {
    Primitive,
    Composite(ServiceCompositeType),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceCompositeType {
    pub fields: Vec<ServiceField>,
    pub is_input: bool,
    pub access: Access,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceField {
    pub name: String,
    pub typ: FieldType<ServiceFieldType>,
    pub has_default_value: bool, // does this field have a default value?
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceFieldType {
    pub type_id: SerializableSlabIndex<ServiceType>,
    pub type_name: String,
}

impl Named for ServiceFieldType {
    fn name(&self) -> &str {
        &self.type_name
    }
}

impl TypeDefinitionProvider<SerializableSlab<ServiceType>> for ServiceType {
    fn type_definition(&self, service_types: &SerializableSlab<ServiceType>) -> TypeDefinition {
        match &self.kind {
            ServiceTypeKind::Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(&self.name),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
            ServiceTypeKind::Composite(ServiceCompositeType {
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
                            default_positioned(model_field.field_definition(service_types))
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

impl FieldDefinitionProvider<SerializableSlab<ServiceType>> for ServiceField {
    fn field_definition(&self, _service_types: &SerializableSlab<ServiceType>) -> FieldDefinition {
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

impl Parameter for ServiceField {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }
}
