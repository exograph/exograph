use std::vec;

use async_graphql_parser::types::{
    BaseType, FieldDefinition, InputValueDefinition, ObjectType, Type, TypeDefinition, TypeKind,
};
use async_graphql_value::Name;
use core_model::type_normalization::{
    FieldDefinitionProvider, TypeDefinitionProvider, default_positioned, default_positioned_name,
};

use crate::subsystem::PostgresGraphQLSubsystem;

use postgres_core_model::vector_distance::{
    VectorDistanceField, VectorDistanceType, VectorDistanceTypeField,
};

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
