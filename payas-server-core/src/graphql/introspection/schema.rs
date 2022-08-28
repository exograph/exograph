use async_graphql_parser::types::{BaseType, Type};

use async_graphql_value::{ConstValue, Name};
use payas_model::model::system::ModelSystem;

use super::definition::{
    provider::{FieldDefinitionProvider, TypeDefinitionProvider},
    type_introspection::TypeDefinitionIntrospection,
};
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
    pub fn new(system: &ModelSystem) -> Schema {
        let mut type_definitions: Vec<SchemaTypeDefinition> = system
            .types
            .iter()
            .map(|model_type| model_type.1.type_definition(system))
            .collect();

        let argument_type_definitions: Vec<SchemaTypeDefinition> = system
            .argument_types
            .iter()
            .map(|m| m.1.type_definition(system))
            .collect();

        let order_by_param_type_definitions: Vec<SchemaTypeDefinition> = system
            .order_by_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(system))
            .collect();

        let predicate_param_type_definitions: Vec<SchemaTypeDefinition> = system
            .predicate_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(system))
            .collect();

        let mutation_param_type_definitions: Vec<SchemaTypeDefinition> = system
            .mutation_types
            .iter()
            .map(|parameter_type| parameter_type.1.type_definition(system))
            .collect();

        let query_type_definition = {
            let fields: Vec<_> = system
                .queries
                .values
                .iter()
                .map(|query| query.1.field_definition(system))
                .collect();

            // Even though we resolve __type and __schema fields for the Query
            // type, GraphQL spec doesn't allow them to be exposed as an
            // ordinary field. Therefore, we have to treat them specially (see
            // SelectionSetValidator::validate_field)
            SchemaTypeDefinition {
                extend: false,
                description: None,
                name: Name::new(QUERY_ROOT_TYPENAME),
                kind: SchemaTypeKind::Object(SchemaObjectType {
                    implements: vec![],
                    fields,
                }),
            }
        };

        let mutation_type_definition = {
            let fields = system
                .mutations
                .values
                .iter()
                .map(|mutation| mutation.1.field_definition(system))
                .collect();

            SchemaTypeDefinition {
                extend: false,
                description: None,
                name: Name::new(MUTATION_ROOT_TYPENAME),
                kind: SchemaTypeKind::Object(SchemaObjectType {
                    implements: vec![],
                    fields,
                }),
            }
        };

        type_definitions.push(Self::create_schema_type_definition());
        type_definitions.push(Self::create_type_definition());
        type_definitions.push(Self::create_field_definition());
        type_definitions.push(Self::create_directive_definition());
        type_definitions.push(Self::create_directive_location_definition());
        type_definitions.push(Self::create_input_value_definition());

        type_definitions.push(query_type_definition);
        type_definitions.push(mutation_type_definition);
        type_definitions.extend(argument_type_definitions);
        type_definitions.extend(order_by_param_type_definitions);
        type_definitions.extend(predicate_param_type_definitions);
        type_definitions.extend(mutation_param_type_definitions);

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
            .find(|td| td.name().as_str() == type_name)
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
            kind: SchemaTypeKind::Scalar,
        }
    }

    fn create_input_value_definition() -> SchemaTypeDefinition {
        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new("__InputValue"),
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
        }
    }
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
}
