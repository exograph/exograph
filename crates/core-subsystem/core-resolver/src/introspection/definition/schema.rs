// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashSet, sync::Arc};

use async_graphql_parser::{
    types::{
        BaseType, FieldDefinition, InputValueDefinition, ObjectType, Type, TypeDefinition, TypeKind,
    },
    Positioned,
};

use async_graphql_value::Name;
use core_model::{
    primitive_type::PrimitiveType,
    type_normalization::{
        default_positioned, default_positioned_name, TypeDefinitionIntrospection,
    },
};

use crate::{plugin::SubsystemGraphQLResolver, validation::underlying_type};

use super::scope::{SchemaScope, SchemaScopeFilter};

#[derive(Debug, Clone)]
pub struct Schema {
    pub type_definitions: Vec<TypeDefinition>,
    pub(crate) schema_field_definition: FieldDefinition,
    pub(crate) type_field_definition: FieldDefinition,
    pub declaration_doc_comments: Arc<Option<String>>,
}

pub const QUERY_ROOT_TYPENAME: &str = "Query";
pub const MUTATION_ROOT_TYPENAME: &str = "Mutation";
pub const SUBSCRIPTION_ROOT_TYPENAME: &str = "Subscription";

impl Schema {
    pub fn new_from_resolvers(
        subsystem_resolvers: &[Arc<dyn SubsystemGraphQLResolver + Send + Sync>],
        scope: SchemaScope,
        declaration_doc_comments: Arc<Option<String>>,
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

        let model_matches = |field_defn: &FieldDefinition, filter: &SchemaScopeFilter| {
            let field_return_type = underlying_type(&field_defn.ty.node);
            PrimitiveType::is_primitive(field_return_type) || filter.matches(field_return_type)
        };

        let name_matches = |field_defn: &FieldDefinition, filter: &SchemaScopeFilter| {
            filter.matches(&field_defn.name.node)
        };

        let queries = {
            let mut queries = subsystem_resolvers
                .iter()
                .fold(vec![], |mut acc, resolver| {
                    acc.extend(
                        resolver
                            .schema_queries()
                            .into_iter()
                            .filter(|q| {
                                model_matches(q, &scope.query_entities)
                                    && name_matches(q, &scope.query_names)
                            })
                            .collect::<Vec<FieldDefinition>>(),
                    );
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
                    acc.extend(
                        resolver
                            .schema_mutations()
                            .into_iter()
                            .filter(|m| {
                                model_matches(m, &scope.mutation_entities)
                                    && name_matches(m, &scope.mutation_names)
                            })
                            .collect::<Vec<FieldDefinition>>(),
                    );
                    acc
                });

            // ensure introspection outputs mutations in a stable order
            mutations.sort_by_key(|m| m.name.clone());
            mutations
        };

        Self::new(
            type_definitions,
            queries,
            mutations,
            declaration_doc_comments,
        )
    }

    pub(crate) fn new(
        type_definitions: Vec<TypeDefinition>,
        queries: Vec<FieldDefinition>,
        mutations: Vec<FieldDefinition>,
        declaration_doc_comments: Arc<Option<String>>,
    ) -> Schema {
        let mut type_definitions = type_definitions;

        // GraphQL spec requires a `Query` type to be present in the schema. Per
        // https://spec.graphql.org/June2018/#sec-Root-Operation-Types: "The query root operation
        // type must be provided and must be an Object type." However, we may not have any queries
        // (for example, with an empty model). Introspection will fail in such cases. We handle in
        // our playground by detecting this situation printing a message.
        //
        // Even though we resolve __type and __schema fields for the Query type, GraphQL spec
        // doesn't allow them to be exposed as an ordinary field. Therefore, we have to treat them
        // specially (see SelectionSetValidator::validate_field)
        if !queries.is_empty() {
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
        }

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

        {
            // The way we build types (queries, mutations, their parameters, etc.) ends up
            // creating a lot of types that are not used by the schema. We need to remove
            // them to avoid unnecessary bloat in the introspection result.
            //
            // We start by visiting all the types that are used by the schema.
            // Start with the root types and recursively visit all the types that are used by them.
            // Then, retain only the types that are used by the schema.
            let mut used_types = HashSet::new();

            for root_type in &[
                QUERY_ROOT_TYPENAME,
                MUTATION_ROOT_TYPENAME,
                SUBSCRIPTION_ROOT_TYPENAME,
                "__Schema",
                "__Type",
            ] {
                Self::get_used_types(root_type, &type_definitions, &mut used_types);
            }

            type_definitions.retain(|td| used_types.contains(td.name.node.as_str()));

            // de-duplicate types (we have multiple types with the same name such as Boolean)
            type_definitions.dedup_by_key(|td| td.name.node.as_str().to_string());
        }

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
            declaration_doc_comments,
        }
    }

    fn get_used_types(
        root_type_name: &str,
        type_definitions: &Vec<TypeDefinition>,
        used_types: &mut HashSet<String>,
    ) {
        fn get_type_definition<'a>(
            type_definitions: &'a [TypeDefinition],
            type_name: &str,
        ) -> Option<&'a TypeDefinition> {
            type_definitions
                .iter()
                .find(|td| td.name.node.as_str() == type_name)
        }

        if used_types.contains(root_type_name) {
            return;
        }

        used_types.insert(root_type_name.to_string());
        let root_type = get_type_definition(type_definitions, root_type_name);

        if let Some(root_type) = root_type {
            match &root_type.kind {
                TypeKind::Object(object_type) => {
                    for field in &object_type.fields {
                        Self::get_used_types(
                            underlying_type(&field.node.ty.node).as_str(),
                            type_definitions,
                            used_types,
                        );
                        // used_types
                        //     .insert(underlying_type(&field.node.ty.node).as_str().to_string());
                        for arg in &field.node.arguments {
                            let arg_type_name = underlying_type(&arg.node.ty.node).as_str();
                            Self::get_used_types(arg_type_name, type_definitions, used_types);
                        }
                    }
                }
                TypeKind::Interface(interface_type) => {
                    for field in &interface_type.fields {
                        Self::get_used_types(
                            underlying_type(&field.node.ty.node).as_str(),
                            type_definitions,
                            used_types,
                        );
                    }
                }
                TypeKind::Scalar => {}
                TypeKind::Union(_) => {}
                TypeKind::Enum(_) => {}
                TypeKind::InputObject(object_type) => {
                    for field in &object_type.fields {
                        // used_types
                        //     .insert(underlying_type(&field.node.ty.node).as_str().to_string());
                        let arg_type_name = underlying_type(&field.node.ty.node).as_str();
                        Self::get_used_types(arg_type_name, type_definitions, used_types);
                    }
                }
            }
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
