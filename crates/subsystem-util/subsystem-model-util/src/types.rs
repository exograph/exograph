use async_graphql_parser::types::{
    FieldDefinition, InputObjectType, InputValueDefinition, ObjectType, TypeDefinition, TypeKind,
};

use core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
        TypeDefinitionProvider, TypeModifier,
    },
    types::{DecoratedType, Named},
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ServiceTypeModifier {
    Optional,
    NonNull,
    List,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceField {
    pub name: String,
    pub typ: DecoratedType<ServiceFieldType>,
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

impl From<&ServiceTypeModifier> for TypeModifier {
    fn from(modifier: &ServiceTypeModifier) -> Self {
        match modifier {
            ServiceTypeModifier::Optional => TypeModifier::Optional,
            ServiceTypeModifier::NonNull => TypeModifier::NonNull,
            ServiceTypeModifier::List => TypeModifier::List,
        }
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
        let field_type = default_positioned(self.typ.to_introspection_type());

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments: vec![],
            ty: field_type,
            directives: vec![],
        }
    }
}

// We need to a special case for the GqlField type, so that we can properly
// created nested types such as Optional(List(List(String))). The blanket impl
// above will not work for nested types like these.
impl InputValueProvider for ServiceField {
    fn input_value(&self) -> InputValueDefinition {
        let field_type = default_positioned(self.typ.to_introspection_type());

        InputValueDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            ty: field_type,
            default_value: None,
            directives: vec![],
        }
    }
}
