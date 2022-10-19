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

use crate::plugin::SubsystemResolver;

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
            subsystem_resolvers
                .iter()
                .fold(vec![], |mut acc, resolver| {
                    acc.extend(resolver.schema_types());
                    acc
                })
        };

        let queries = {
            subsystem_resolvers
                .iter()
                .fold(vec![], |mut acc, resolver| {
                    acc.extend(resolver.schema_queries());
                    acc
                })
        };

        let mutations = {
            subsystem_resolvers
                .iter()
                .fold(vec![], |mut acc, resolver| {
                    acc.extend(resolver.schema_mutations());
                    acc
                })
        };

        Self::new(type_definitions, queries, mutations)
    }

    pub fn new(
        type_definitions: Vec<TypeDefinition>,
        queries: Vec<Positioned<FieldDefinition>>,
        mutations: Vec<Positioned<FieldDefinition>>,
    ) -> Schema {
        let mut type_definitions = type_definitions;

        // Ideally, we should surround it with `if !queries.is_empty() {` (like we do for mutations next)
        // but GraphQL spec requires a `Query` type to be present in the schema.
        // https://spec.graphql.org/June2018/#sec-Root-Operation-Types
        // "The query root operation type must be provided and must be an Object type."
        // So we always add a `Query` type to the schema. This means introspection will fail
        // if there are no queries. See https://github.com/payalabs/payas/issues/480
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
                fields: queries,
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

        // strings and booleans are required for introspection, so validation will fail without them present
        // add a base set of scalars that should always be supported
        let mut create_primitive_if_not_present = |type_name: &str| {
            if !type_definitions
                .iter()
                .any(|typedef| typedef.name.node.as_str() == type_name)
            {
                type_definitions.push(TypeDefinition {
                    extend: false,
                    description: None,
                    name: default_positioned_name(type_name),
                    directives: vec![],
                    kind: TypeKind::Scalar,
                });
            }
        };

        create_primitive_if_not_present("Boolean");
        create_primitive_if_not_present("String");

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
