use crate::model::ModelPostgresSystem;

use super::column_path::ColumnIdPathLink;
use async_graphql_parser::{
    types::{EnumType, EnumValueDefinition, InputObjectType, TypeDefinition, TypeKind},
    Pos, Positioned,
};
use async_graphql_value::Name;
use core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{
        default_positioned, default_positioned_name, InputValueProvider, Parameter, ParameterType,
        TypeDefinitionProvider, TypeModifier,
    },
};

use super::types::PostgresTypeModifier;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderByParameter {
    pub name: String,
    pub type_name: String,
    pub typ: OrderByParameterTypeWithModifier,

    /// How does this parameter relates with the parent parameter?
    /// For example for parameter used as {order_by: {venue1: {id: Desc}}}, we will have following column links:
    /// id: Some((<the venues.id column>, None))
    /// venue1: Some((<the concerts.venue1_id column>, <the venues.id column>))
    /// order_by: None
    pub column_path_link: Option<ColumnIdPathLink>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderByParameterTypeWithModifier {
    pub type_id: SerializableSlabIndex<OrderByParameterType>,
    pub type_modifier: PostgresTypeModifier,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderByParameterType {
    pub name: String,
    pub kind: OrderByParameterTypeKind,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OrderByParameterTypeKind {
    Primitive,
    Composite { parameters: Vec<OrderByParameter> },
}

pub const PRIMITIVE_ORDERING_OPTIONS: [&str; 2] = ["ASC", "DESC"];

impl ParameterType for OrderByParameterType {
    fn name(&self) -> &String {
        &self.name
    }
}

impl Parameter for OrderByParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_modifier(&self) -> TypeModifier {
        (&self.typ.type_modifier).into()
    }
}

impl TypeDefinitionProvider<ModelPostgresSystem> for OrderByParameterType {
    fn type_definition(&self, _system: &ModelPostgresSystem) -> TypeDefinition {
        match &self.kind {
            OrderByParameterTypeKind::Composite { parameters } => {
                let fields = parameters
                    .iter()
                    .map(|parameter| default_positioned(parameter.input_value()))
                    .collect();

                TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(self.name()),
                    directives: vec![],
                    kind: TypeKind::InputObject(InputObjectType { fields }),
                }
            }
            OrderByParameterTypeKind::Primitive => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(self.name()),
                directives: vec![],
                kind: TypeKind::Enum(EnumType {
                    values: PRIMITIVE_ORDERING_OPTIONS
                        .iter()
                        .map(|value| {
                            Positioned::new(
                                EnumValueDefinition {
                                    description: None,
                                    value: Positioned::new(Name::new(value), Pos::default()),
                                    directives: vec![],
                                },
                                Pos::default(),
                            )
                        })
                        .collect(),
                }),
            },
        }
    }
}
