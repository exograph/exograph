use async_graphql_parser::{
    types::{InputObjectType, InputValueDefinition, TypeDefinition, TypeKind},
    Positioned,
};
use payas_core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{
        default_positioned_name, Parameter, TypeDefinitionIntrospection, TypeDefinitionProvider,
        TypeModifier,
    },
};
use serde::{Deserialize, Serialize};

use crate::model::ModelServiceSystem;

use super::types::{ServiceType, ServiceTypeModifier};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameter {
    pub name: String,
    pub typ: ArgumentParameterType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArgumentParameterType {
    pub name: String,
    pub type_modifier: ServiceTypeModifier,
    pub type_id: Option<SerializableSlabIndex<ServiceType>>,
    pub is_primitive: bool,
}

impl Parameter for ArgumentParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.typ.name
    }

    fn type_modifier(&self) -> TypeModifier {
        (&self.typ.type_modifier).into()
    }
}

// TODO: Reduce duplication from the above impl
impl TypeDefinitionProvider<ModelServiceSystem> for ArgumentParameterType {
    fn type_definition(&self, system: &ModelServiceSystem) -> TypeDefinition {
        let type_def = system
            .service_types
            .get(self.type_id.unwrap())
            .unwrap()
            .type_definition(system);

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

impl TypeDefinitionProvider<ModelServiceSystem> for ArgumentParameter {
    fn type_definition(&self, _system: &ModelServiceSystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}
