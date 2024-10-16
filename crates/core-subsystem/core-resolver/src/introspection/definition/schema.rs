// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};

use async_graphql_parser::types::{BaseType, Type};

use async_graphql_value::{ConstValue, Name};

use crate::{plugin::SubsystemResolver, validation::underlying_type};

#[derive(Debug, Clone)]
pub struct Schema {
    pub type_definitions: Vec<SchemaTypeDefinition>,
    pub(crate) schema_field_definition: SchemaFieldDefinition,
    pub(crate) type_field_definition: SchemaFieldDefinition,
}

pub const QUERY_ROOT_TYPENAME: &str = "Query";
pub const MUTATION_ROOT_TYPENAME: &str = "Mutation";
pub const SUBSCRIPTION_ROOT_TYPENAME: &str = "Subscription";

impl Schema {
    pub fn new_from_resolvers(
        subsystem_resolvers: &[Box<dyn SubsystemResolver + Send + Sync>],
    ) -> Schema {
        let type_definitions: Vec<SchemaTypeDefinition> = {
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
        type_definitions: Vec<SchemaTypeDefinition>,
        queries: Vec<SchemaFieldDefinition>,
        mutations: Vec<SchemaFieldDefinition>,
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
            type_definitions.push(SchemaTypeDefinition {
                extend: false,
                description: None,
                name: Name::new(QUERY_ROOT_TYPENAME),
                directives: vec![],
                kind: SchemaTypeKind::Object(SchemaObjectType {
                    implements: vec![],
                    fields: queries,
                }),
            });
        }

        if !mutations.is_empty() {
            type_definitions.push(SchemaTypeDefinition {
                extend: false,
                description: None,
                name: Name::new(MUTATION_ROOT_TYPENAME),
                directives: vec![],
                kind: SchemaTypeKind::Object(SchemaObjectType {
                    implements: vec![],
                    fields: mutations,
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
            .partition(|td| matches!(td.kind, SchemaTypeKind::Scalar));

        // Start with assuming no scalars are used
        let mut unused_scalars_names: HashSet<String> = scalars
            .iter()
            .map(|td| td.name.as_str().to_string())
            .collect();

        fn remove_scalars(
            field_definitions: &Vec<SchemaFieldDefinition>,
            unused_scalars_names: &mut HashSet<String>,
        ) {
            for field in field_definitions {
                unused_scalars_names.remove(underlying_type(&field.ty).as_str());
                for arg in &field.arguments {
                    unused_scalars_names.remove(underlying_type(&arg.ty).as_str());
                }
            }
        }

        // Next, remove scalars that are used
        for td in &type_definitions {
            match &td.kind {
                SchemaTypeKind::Object(object_type) => {
                    remove_scalars(&object_type.fields, &mut unused_scalars_names);
                }
                SchemaTypeKind::Interface(interface_type) => {
                    remove_scalars(&interface_type.fields, &mut unused_scalars_names);
                }
                SchemaTypeKind::InputObject(input_object_type) => {
                    for field in &input_object_type.fields {
                        unused_scalars_names.remove(underlying_type(&field.ty).as_str());
                    }
                }
                _ => {}
            }
        }

        let used_scalars = scalars
            .into_iter()
            .filter(|td| !unused_scalars_names.contains(td.name.as_str()));
        // Create a unique list of scalars (each subsystem may expose the same scalar)
        let unique_scalars: HashMap<_, _> = used_scalars
            .into_iter()
            .map(|td| (td.name.as_str().to_string(), td))
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
            ),
            type_field_definition: Self::create_field(
                "__type",
                true,
                None,
                "__Type",
                vec![SchemaInputValueDefinition {
                    description: None,
                    name: Name::new("name"),
                    directives: vec![],
                    default_value: None,
                    ty: Type {
                        base: BaseType::Named(Name::new("String")),
                        nullable: true,
                    },
                }],
            ),
        }
    }

    pub fn get_type_definition(&self, type_name: &str) -> Option<&SchemaTypeDefinition> {
        self.type_definitions
            .iter()
            .find(|td| td.name.as_str() == type_name)
    }

    fn create_schema_type_definition() -> SchemaTypeDefinition {
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
            .map(|field_name| SchemaFieldDefinition {
                description: None,
                name: Name::new(field_name),
                arguments: vec![],
                ty: Type {
                    base: BaseType::Named(Name::new("__Type")),
                    nullable: false,
                },
                directives: vec![],
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

        SchemaTypeDefinition {
            extend: false,
            description: Some("The current type schema of this server.".to_string()),
            name: Name::new("__Schema"),
            directives: vec![],
            kind: SchemaTypeKind::Object(SchemaObjectType {
                implements: vec![],
                fields,
            }),
        }
    }

    fn create_type_definition() -> SchemaTypeDefinition {
        let fields_arguments = vec![SchemaInputValueDefinition {
            description: None,
            name: Name::new("includeDeprecated"),
            directives: vec![],
            default_value: None,
            ty: Type {
                base: BaseType::Named(Name::new("Boolean")),
                nullable: true,
            },
        }];

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

        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new("__Type"),
            directives: vec![],
            kind: SchemaTypeKind::Object(SchemaObjectType {
                implements: vec![],
                fields,
            }),
        }
    }

    fn create_field_definition() -> SchemaTypeDefinition {
        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new("__Field"),
            directives: vec![],
            kind: SchemaTypeKind::Object(SchemaObjectType {
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

    fn create_directive_definition() -> SchemaTypeDefinition {
        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new("__Directive"),
            directives: vec![],
            kind: SchemaTypeKind::Object(SchemaObjectType {
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

    fn create_directive_location_definition() -> SchemaTypeDefinition {
        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new("__DirectiveLocation"),
            directives: vec![],
            kind: SchemaTypeKind::Scalar,
        }
    }

    fn create_input_value_definition() -> SchemaTypeDefinition {
        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new("__InputValue"),
            directives: vec![],
            kind: SchemaTypeKind::Object(SchemaObjectType {
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
        arguments: Vec<SchemaInputValueDefinition>,
    ) -> SchemaFieldDefinition {
        SchemaFieldDefinition {
            description: description.map(|d| d.to_string()),
            name: Name::new(name),
            arguments,
            ty: Type {
                base: BaseType::Named(Name::new(element_type)),
                nullable,
            },
            directives: vec![],
        }
    }

    fn create_list_field(
        name: &str,
        nullable: bool,
        description: Option<&str>,
        element_type: &str,
        arguments: Vec<SchemaInputValueDefinition>,
    ) -> SchemaFieldDefinition {
        SchemaFieldDefinition {
            description: description.map(|d| d.to_string()),
            name: Name::new(name),
            arguments,
            ty: Type {
                base: BaseType::List(Box::new(Type {
                    base: BaseType::Named(Name::new(element_type)),
                    nullable,
                })),
                nullable,
            },
            directives: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchemaInterfaceType {
    pub implements: Vec<Name>,
    pub fields: Vec<SchemaFieldDefinition>,
}

#[derive(Debug, Clone)]
pub struct SchemaConstDirective {
    pub name: Name,
    pub arguments: Vec<(Name, ConstValue)>,
}

#[derive(Debug, Clone)]
pub struct SchemaTypeDefinition {
    /// Whether the type is an extension of another type.
    pub extend: bool,
    /// The description of the type, if present. This is never present on an
    /// extension type.
    pub description: Option<String>,
    /// The name of the type.
    pub name: Name,
    /// Which kind of type is being defined; scalar, object, enum, etc.
    pub kind: SchemaTypeKind,
    /// The directives of type definition.
    pub directives: Vec<SchemaConstDirective>,
}

#[derive(Debug, Clone)]
pub enum SchemaTypeKind {
    /// A scalar type.
    Scalar,
    /// An object type.
    Object(SchemaObjectType),
    /// An enum type.
    Enum(SchemaEnumType),
    /// An input object type.
    InputObject(SchemaInputObjectType),
    /// An interface type.
    Interface(SchemaInterfaceType),
}

#[derive(Debug, Clone)]
pub struct SchemaEnumType {
    /// The possible values of the enum.
    pub values: Vec<SchemaEnumValueDefinition>,
}

#[derive(Debug, Clone)]
pub struct SchemaEnumValueDefinition {
    /// The description of the argument.
    pub description: Option<String>,
    /// The value name.
    pub value: Name,
}

#[derive(Debug, Clone)]
pub struct SchemaObjectType {
    /// The interfaces implemented by the object.
    pub implements: Vec<Name>,
    /// The fields of the object type.
    pub fields: Vec<SchemaFieldDefinition>,
}

#[derive(Debug, Clone)]
pub struct SchemaInputObjectType {
    /// The fields of the input object.
    pub fields: Vec<SchemaInputValueDefinition>,
}

/// The definition of an input value inside the arguments of a field.
///
/// [Reference](https://spec.graphql.org/October2021/#InputValueDefinition).
#[derive(Debug, Clone)]
pub struct SchemaInputValueDefinition {
    /// The description of the argument.
    pub description: Option<String>,
    /// The name of the argument.
    pub name: Name,
    /// The type of the argument.
    pub ty: Type,
    /// The default value of the argument, if there is one.
    pub default_value: Option<ConstValue>,
    /// The directives of the input value.
    pub directives: Vec<SchemaConstDirective>,
}

#[derive(Debug, Clone)]
pub struct SchemaFieldDefinition {
    /// The description of the field.
    pub description: Option<String>,
    /// The name of the field.
    pub name: Name,
    /// The arguments of the field.
    pub arguments: Vec<SchemaInputValueDefinition>,
    /// The type of the field.
    pub ty: Type,
    /// The directives of the field.
    pub directives: Vec<SchemaConstDirective>,
}
