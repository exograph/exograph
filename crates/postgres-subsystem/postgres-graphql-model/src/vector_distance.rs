use std::vec;

use async_graphql_parser::types::{
    BaseType, FieldDefinition, InputValueDefinition, ObjectType, Type, TypeDefinition, TypeKind,
};
use async_graphql_value::Name;
use core_plugin_interface::core_model::type_normalization::{
    default_positioned, default_positioned_name, FieldDefinitionProvider, TypeDefinitionProvider,
};
use exo_sql::{ColumnId, VectorDistanceFunction};
use serde::{Deserialize, Serialize};

use crate::subsystem::PostgresGraphQLSubsystem;

use postgres_core_model::access::Access;

/// Field for a vector distance function
/// Represents:
/// ```graphql
/// document {
///    contentVector: [Float!]!
///    contentVectorDistance(to: [Float!]!): Float! <--- This is the field
/// }
/// ```
#[derive(Serialize, Deserialize, Debug)]
pub struct VectorDistanceField {
    pub name: String,
    pub column_id: ColumnId,
    pub size: usize,
    pub distance_function: VectorDistanceFunction,
    pub access: Access,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VectorDistanceType {
    pub name: String, // name of the type, currently always "VectorDistance", but we could introduce `VectorDistance64`, etc. in the future
    fields: Vec<VectorDistanceTypeField>,
}

impl VectorDistanceType {
    pub fn new(name: String) -> Self {
        Self {
            name,
            fields: vec![VectorDistanceTypeField {}],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VectorDistanceTypeField {}

/// The definition of a vector distance field. Takes and argument
impl FieldDefinitionProvider<PostgresGraphQLSubsystem> for VectorDistanceField {
    fn field_definition(&self, _system: &PostgresGraphQLSubsystem) -> FieldDefinition {
        // {to: [Float!]!}
        let argument = InputValueDefinition {
            description: None,
            name: default_positioned_name("to"),
            ty: default_positioned(Type {
                base: BaseType::List(Box::new(Type {
                    base: BaseType::Named(Name::new("Float")),
                    nullable: false,
                })),
                nullable: false,
            }),
            default_value: None,
            directives: vec![],
        };

        FieldDefinition {
            description: None,
            name: default_positioned_name(&self.name),
            arguments: vec![default_positioned(argument)],
            ty: default_positioned(Type {
                base: BaseType::Named(Name::new("Float")),
                nullable: false,
            }),
            directives: vec![],
        }
    }
}

impl TypeDefinitionProvider<PostgresGraphQLSubsystem> for VectorDistanceType {
    fn type_definition(&self, system: &PostgresGraphQLSubsystem) -> TypeDefinition {
        let kind = {
            let fields: Vec<_> = self
                .fields
                .iter()
                .map(|field| default_positioned(field.field_definition(system)))
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

impl FieldDefinitionProvider<PostgresGraphQLSubsystem> for VectorDistanceTypeField {
    fn field_definition(&self, _system: &PostgresGraphQLSubsystem) -> FieldDefinition {
        FieldDefinition {
            description: None,
            name: default_positioned_name("distance"),
            arguments: vec![],
            ty: default_positioned(Type {
                base: BaseType::Named(Name::new("Float")),
                nullable: false,
            }),
            directives: vec![],
        }
    }
}
