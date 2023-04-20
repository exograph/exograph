// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};

use async_graphql_parser::{
    types::{
        BaseType, FieldDefinition, InputValueDefinition, ObjectType, Type, TypeDefinition, TypeKind,
    },
    Positioned,
};

use async_graphql_value::Name;
use core_model::type_normalization::{
    default_positioned, default_positioned_name, TypeDefinitionIntrospection,
};

use crate::{plugin::SubsystemResolver, validation::underlying_type};

#[derive(Debug, Clone)]
pub struct Schema {
    pub type_definitions: Vec<TypeDefinition>,
    pub(crate) schema_field_definition: FieldDefinition,
    pub(crate) type_field_definition: FieldDefinition,
}

pub const QUERY_ROOT_TYPENAME: &str = "Query";
pub const MUTATION_ROOT_TYPENAME: &str = "Mutation";
pub const SUBSCRIPTION_ROOT_TYPENAME: &str = "Subscription";

impl Schema {
    pub fn new_from_resolvers(
        subsystem_resolvers: &[Box<dyn SubsystemResolver + Send + Sync>],
    ) -> Schema {
        let type_definitions: Vec<TypeDefinition> = {
            let mut typedefs = subsystem_resolvers
                .iter()
                .fold(vec![], |mut acc, resolver| {
                    acc.extend(resolver.schema_types());
                    acc
                });

            // ensure introspection outputs fields in a stable order
            typedefs.sort_by_key(|f| f.name.clone());
            typedefs
        };

        let queries = {
            let mut queries = subsystem_resolvers
                .iter()
                .fold(vec![], |mut acc, resolver| {
                    acc.extend(resolver.schema_queries());
                    acc
                });

            // ensure introspection outputs queries in a stable order
            queries.sort_by_key(|q| q.name.clone());
            queries
        };

        let mutations = {
            let mut mutations = subsystem_resolvers
                .iter()
                .fold(vec![], |mut acc, resolver| {
                    acc.extend(resolver.schema_mutations());
                    acc
                });

            // ensure introspection outputs mutations in a stable order
            mutations.sort_by_key(|m| m.name.clone());
            mutations
        };

        Self::new(type_definitions, queries, mutations)
    }

    pub fn new(
        type_definitions: Vec<TypeDefinition>,
        queries: Vec<FieldDefinition>,
        mutations: Vec<FieldDefinition>,
    ) -> Schema {
        let mut type_definitions = type_definitions;

        // Ideally, we should surround it with `if !queries.is_empty() {` (like we do for mutations next)
        // but GraphQL spec requires a `Query` type to be present in the schema.
        // https://spec.graphql.org/June2018/#sec-Root-Operation-Types
        // "The query root operation type must be provided and must be an Object type."
        // So we always add a `Query` type to the schema. This means introspection will fail
        // if there are no queries. See https://github.com/exograph/exograph/issues/480
        //
        // Even though we resolve __type and __schema fields for the Query
        // type, GraphQL spec doesn't allow them to be exposed as an
        // ordinary field. Therefore, we have to treat them specially (see
        // SelectionSetValidator::validate_field)
        type_definitions.push(TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(QUERY_ROOT_TYPENAME),
            directives: vec![],
            kind: TypeKind::Object(ObjectType {
                implements: vec![],
                fields: queries.into_iter().map(default_positioned).collect(),
            }),
        });

        if !mutations.is_empty() {
            type_definitions.push(TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(MUTATION_ROOT_TYPENAME),
                directives: vec![],
                kind: TypeKind::Object(ObjectType {
                    implements: vec![],
                    fields: mutations.into_iter().map(default_positioned).collect(),
                }),
            });
        };

        type_definitions.push(Self::create_schema_type_definition());
        type_definitions.push(Self::create_type_definition());
        type_definitions.push(Self::create_field_definition());
        type_definitions.push(Self::create_directive_definition());
        type_definitions.push(Self::create_directive_location_definition());
        type_definitions.push(Self::create_input_value_definition());

        // We may have unused scalars (an artifact of how our typechecking starts with supplying all scalars)
        // So here we remove unused ones
        let (scalars, mut type_definitions): (Vec<_>, Vec<_>) = type_definitions
            .into_iter()
            .partition(|td| matches!(td.kind, TypeKind::Scalar));

        // Start with assuming no scalars are used
        let mut unused_scalars_names: HashSet<String> = scalars
            .iter()
            .map(|td| td.name.node.as_str().to_string())
            .collect();

        // Next, remove scalars that are used
        for td in &type_definitions {
            match &td.kind {
                TypeKind::Object(object_type) => {
                    for field in &object_type.fields {
                        unused_scalars_names.remove(underlying_type(&field.node.ty.node).as_str());
                    }
                }
                TypeKind::Interface(interface_type) => {
                    for field in &interface_type.fields {
                        unused_scalars_names.remove(underlying_type(&field.node.ty.node).as_str());
                    }
                }
                TypeKind::InputObject(input_object_type) => {
                    for field in &input_object_type.fields {
                        unused_scalars_names.remove(underlying_type(&field.node.ty.node).as_str());
                    }
                }
                _ => {}
            }
        }

        let used_scalars = scalars
            .into_iter()
            .filter(|td| !unused_scalars_names.contains(td.name.node.as_str()));
        // Create a unique list of scalars (each subsystem may expose the same scalar)
        let unique_scalars: HashMap<_, _> = used_scalars
            .into_iter()
            .map(|td| (td.name.node.as_str().to_string(), td))
            .collect();

        type_definitions.extend(unique_scalars.into_values());

        Schema {
            type_definitions,
            schema_field_definition: Self::create_field(
                "__schema",
                false,
                Some("Access the current type schema of this server."),
                "__Schema",
                vec![],
            )
            .node,
            type_field_definition: Self::create_field(
                "__type",
                true,
                None,
                "__Type",
                vec![default_positioned(InputValueDefinition {
                    description: None,
                    name: default_positioned_name("name"),
                    directives: vec![],
                    default_value: None,
                    ty: default_positioned(Type {
                        base: BaseType::Named(Name::new("String")),
                        nullable: true,
                    }),
                })],
            )
            .node,
        }
    }

    pub fn get_type_definition(&self, type_name: &str) -> Option<&TypeDefinition> {
        self.type_definitions
            .iter()
            .find(|td| td.name().as_str() == type_name)
    }

    fn create_schema_type_definition() -> TypeDefinition {
        let directives_field = Self::create_list_field(
            "directives",
            false,
            Some("A list of the directives supported by this server."),
            "__Directive",
            vec![],
        );

        let types_field = Self::create_list_field(
            "types",
            false,
            Some("A list of the types supported by this server."),
            "__Type",
            vec![],
        );

        let mut fields: Vec<_> = ["queryType", "mutationType", "subscriptionType"]
            .into_iter()
            .map(|field_name| {
                default_positioned(FieldDefinition {
                    description: None,
                    name: default_positioned_name(field_name),
                    arguments: vec![],
                    ty: default_positioned(Type {
                        base: BaseType::Named(Name::new("__Type")),
                        nullable: false,
                    }),
                    directives: vec![],
                })
            })
            .collect();

        fields.push(Self::create_field(
            "description",
            true,
            None,
            "String",
            vec![],
        ));
        fields.push(directives_field);
        fields.push(types_field);

        TypeDefinition {
            extend: false,
            description: Some(default_positioned(
                "The current type schema of this server.".to_string(),
            )),
            name: default_positioned_name("__Schema"),
            directives: vec![],
            kind: TypeKind::Object(ObjectType {
                implements: vec![],
                fields,
            }),
        }
    }

    fn create_type_definition() -> TypeDefinition {
        let fields_arguments = vec![default_positioned(InputValueDefinition {
            description: None,
            name: default_positioned_name("includeDeprecated"),
            directives: vec![],
            default_value: None,
            ty: default_positioned(Type {
                base: BaseType::Named(Name::new("Boolean")),
                nullable: true,
            }),
        })];

        let fields = vec![
            Self::create_field("name", true, None, "String", vec![]),
            Self::create_field("description", true, None, "String", vec![]),
            Self::create_field("kind", false, None, "String", vec![]),
            Self::create_field("specifiedByURL", true, None, "String", vec![]),
            Self::create_field("ofType", false, None, "__Type", vec![]),
            Self::create_list_field("fields", false, None, "__Field", fields_arguments.clone()),
            Self::create_list_field(
                "inputFields",
                false,
                None,
                "__InputValue",
                fields_arguments.clone(),
            ),
            Self::create_list_field("enumValues", false, None, "__InputValue", fields_arguments),
            Self::create_list_field("interfaces", false, None, "__Type", vec![]),
            Self::create_list_field("possibleTypes", false, None, "__Type", vec![]),
        ];

        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name("__Type"),
            directives: vec![],
            kind: TypeKind::Object(ObjectType {
                implements: vec![],
                fields,
            }),
        }
    }

    fn create_field_definition() -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name("__Field"),
            directives: vec![],
            kind: TypeKind::Object(ObjectType {
                implements: vec![],
                fields: vec![
                    Self::create_field("name", true, None, "String", vec![]),
                    Self::create_field("description", true, None, "String", vec![]),
                    Self::create_list_field("args", false, None, "__InputValue", vec![]),
                    Self::create_field("type", false, None, "__Type", vec![]),
                    Self::create_field("isDeprecated", false, None, "Boolean", vec![]),
                    Self::create_field("deprecationReason", true, None, "String", vec![]),
                ],
            }),
        }
    }

    fn create_directive_definition() -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name("__Directive"),
            directives: vec![],
            kind: TypeKind::Object(ObjectType {
                implements: vec![],
                fields: vec![
                    Self::create_field("name", false, None, "String", vec![]),
                    Self::create_field("description", true, None, "String", vec![]),
                    Self::create_field("isRepeatable", false, None, "Boolean", vec![]),
                    Self::create_list_field("args", false, None, "__InputValue", vec![]),
                    Self::create_list_field(
                        "locations",
                        false,
                        None,
                        "__DirectiveLocation",
                        vec![],
                    ),
                ],
            }),
        }
    }

    fn create_directive_location_definition() -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name("__DirectiveLocation"),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }

    fn create_input_value_definition() -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name("__InputValue"),
            directives: vec![],
            kind: TypeKind::Object(ObjectType {
                implements: vec![],
                fields: vec![
                    Self::create_field("name", false, None, "String", vec![]),
                    Self::create_field("description", true, None, "String", vec![]),
                    Self::create_field("type", false, None, "__Type", vec![]),
                    Self::create_field("defaultValue", true, None, "String", vec![]),
                    Self::create_field("isDeprecated", false, None, "Boolean", vec![]),
                    Self::create_field("deprecationReason", true, None, "String", vec![]),
                ],
            }),
        }
    }

    pub fn create_field(
        name: &str,
        nullable: bool,
        description: Option<&str>,
        element_type: &str,
        arguments: Vec<Positioned<InputValueDefinition>>,
    ) -> Positioned<FieldDefinition> {
        default_positioned(FieldDefinition {
            description: description.map(|d| default_positioned(d.to_string())),
            name: default_positioned_name(name),
            arguments,
            ty: default_positioned(Type {
                base: BaseType::Named(Name::new(element_type)),
                nullable,
            }),
            directives: vec![],
        })
    }

    fn create_list_field(
        name: &str,
        nullable: bool,
        description: Option<&str>,
        element_type: &str,
        arguments: Vec<Positioned<InputValueDefinition>>,
    ) -> Positioned<FieldDefinition> {
        default_positioned(FieldDefinition {
            description: description.map(|d| default_positioned(d.to_string())),
            name: default_positioned_name(name),
            arguments,
            ty: default_positioned(Type {
                base: BaseType::List(Box::new(Type {
                    base: BaseType::Named(Name::new(element_type)),
                    nullable,
                })),
                nullable,
            }),
            directives: vec![],
        })
    }
}
