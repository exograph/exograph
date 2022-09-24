use async_graphql_parser::types::{
    BaseType, FieldDefinition, InputObjectType, InputValueDefinition, ObjectType, Type,
    TypeDefinition, TypeKind,
};
use async_graphql_value::Name;

use payas_core_model::{
    mapped_arena::{SerializableSlab, SerializableSlabIndex},
    type_normalization::{
        default_positioned, default_positioned_name, FieldDefinitionProvider, InputValueProvider,
        TypeDefinitionProvider, TypeModifier,
    },
};

use serde::{Deserialize, Serialize};

use crate::{access::Access, model::ModelServiceSystem};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceType {
    pub name: String,
    pub kind: ServiceTypeKind,
    pub is_input: bool, // Is this to be used as an input field (such as an argument in a mutation)? Needed for introspection
}

impl ServiceType {
    pub fn model_fields(&self) -> Vec<&ServiceField> {
        match &self.kind {
            ServiceTypeKind::Primitive => vec![],
            ServiceTypeKind::Composite(ServiceCompositeType { fields, .. }) => {
                fields.iter().collect()
            }
        }
    }

    pub fn model_field(&self, name: &str) -> Option<&ServiceField> {
        self.model_fields()
            .into_iter()
            .find(|model_field| model_field.name == name)
    }

    pub fn is_primitive(&self) -> bool {
        matches!(&self.kind, ServiceTypeKind::Primitive)
    }
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
    pub access: Access,
}

impl ServiceCompositeType {
    pub fn get_field_by_name(&self, name: &str) -> Option<&ServiceField> {
        self.fields.iter().find(|field| field.name == name)
    }
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
    pub typ: ServiceFieldType,
    pub has_default_value: bool, // does this field have a default value?
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceFieldType {
    Optional(Box<ServiceFieldType>),
    Reference {
        type_id: SerializableSlabIndex<ServiceType>,
        is_primitive: bool, // A way to know which arena to look up the type in
        type_name: String,
    },
    List(Box<ServiceFieldType>),
}

impl ServiceFieldType {
    pub fn type_id(&self) -> &SerializableSlabIndex<ServiceType> {
        match self {
            ServiceFieldType::Optional(underlying) | ServiceFieldType::List(underlying) => {
                underlying.type_id()
            }
            ServiceFieldType::Reference { type_id, .. } => type_id,
        }
    }

    pub fn is_primitive(&self) -> bool {
        match self {
            ServiceFieldType::Optional(underlying) | ServiceFieldType::List(underlying) => {
                underlying.is_primitive()
            }
            ServiceFieldType::Reference { is_primitive, .. } => *is_primitive,
        }
    }

    pub fn base_type<'a>(&self, types: &'a SerializableSlab<ServiceType>) -> &'a ServiceType {
        match self {
            ServiceFieldType::Optional(underlying) | ServiceFieldType::List(underlying) => {
                underlying.base_type(types)
            }
            ServiceFieldType::Reference { type_id, .. } => &types[*type_id],
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            ServiceFieldType::Optional(underlying) | ServiceFieldType::List(underlying) => {
                underlying.type_name()
            }
            ServiceFieldType::Reference { type_name, .. } => type_name,
        }
    }

    pub fn optional(&self) -> Self {
        match self {
            ServiceFieldType::Optional(_) => self.clone(),
            _ => ServiceFieldType::Optional(Box::new(self.clone())),
        }
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

impl TypeDefinitionProvider<ModelServiceSystem> for ServiceType {
    fn type_definition(&self, system: &ModelServiceSystem) -> TypeDefinition {
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
                        .map(|model_field| default_positioned(model_field.field_definition(system)))
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

impl FieldDefinitionProvider<ModelServiceSystem> for ServiceField {
    fn field_definition(&self, system: &ModelServiceSystem) -> FieldDefinition {
        let field_type = default_positioned(compute_type(&self.typ));

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments: vec![],
            ty: field_type,
            directives: vec![],
        }
    }
}

pub fn compute_type(typ: &ServiceFieldType) -> Type {
    fn compute_base_type(typ: &ServiceFieldType) -> BaseType {
        match typ {
            ServiceFieldType::Optional(underlying) => compute_base_type(underlying),
            ServiceFieldType::Reference { type_name, .. } => BaseType::Named(Name::new(type_name)),
            ServiceFieldType::List(underlying) => {
                BaseType::List(Box::new(compute_type(underlying)))
            }
        }
    }

    match typ {
        ServiceFieldType::Optional(underlying) => Type {
            base: compute_base_type(underlying),
            nullable: true,
        },
        ServiceFieldType::Reference { type_name, .. } => Type {
            base: BaseType::Named(Name::new(type_name)),
            nullable: false,
        },
        ServiceFieldType::List(underlying) => Type {
            base: BaseType::List(Box::new(compute_type(underlying))),
            nullable: false,
        },
    }
}

// We need to a special case for the GqlField type, so that we can properly
// created nested types such as Optional(List(List(String))). The blanket impl
// above will not work for nested types like these.
impl InputValueProvider for ServiceField {
    fn input_value(&self) -> InputValueDefinition {
        let field_type = default_positioned(compute_type(&self.typ));

        InputValueDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            ty: field_type,
            default_value: None,
            directives: vec![],
        }
    }
}
