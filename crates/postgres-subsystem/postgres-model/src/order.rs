use crate::subsystem::PostgresSubsystem;

use super::column_path::ColumnIdPathLink;
use async_graphql_parser::{
    types::{EnumType, EnumValueDefinition, InputObjectType, Type, TypeDefinition, TypeKind},
    Pos, Positioned,
};
use async_graphql_value::Name;
use core_model::type_normalization::InputValueProvider;
use core_model::types::FieldType;
use core_plugin_interface::core_model::{
    mapped_arena::SerializableSlabIndex,
    type_normalization::{
        default_positioned, default_positioned_name, Parameter, TypeDefinitionProvider,
    },
    types::Named,
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderByParameter {
    pub name: String,
    pub typ: FieldType<OrderByParameterType>,
    pub type_id: SerializableSlabIndex<OrderByParameterType>,

    /// How does this parameter relates with the parent parameter?
    /// For example for parameter used as {order_by: {venue1: {id: Desc}}}, we will have following column links:
    ///   id: Some((<the venues.id column>, None))
    ///   venue1: Some((<the concerts.venue1_id column>, <the venues.id column>))
    ///   order_by: None
    pub column_path_link: Option<ColumnIdPathLink>,
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

impl Named for OrderByParameterType {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Parameter for OrderByParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }
}

impl TypeDefinitionProvider<PostgresSubsystem> for OrderByParameterType {
    fn type_definition(&self, _system: &PostgresSubsystem) -> TypeDefinition {
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
