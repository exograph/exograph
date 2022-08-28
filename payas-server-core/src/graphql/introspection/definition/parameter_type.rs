use async_graphql_value::Name;

use crate::graphql::introspection::{
    definition::type_introspection::TypeDefinitionIntrospection,
    schema::{
        SchemaEnumType, SchemaEnumValueDefinition, SchemaInputObjectType,
        SchemaInputValueDefinition, SchemaTypeDefinition, SchemaTypeKind,
    },
};
use payas_model::model::{
    argument::{ArgumentParameter, ArgumentParameterType},
    limit_offset::{LimitParameter, OffsetParameter},
    order::{OrderByParameterType, OrderByParameterTypeKind},
    predicate::{PredicateParameterType, PredicateParameterTypeKind},
    system::ModelSystem,
};

use super::provider::{InputValueProvider, TypeDefinitionProvider};

pub trait ParameterType {
    fn name(&self) -> &String;
}

impl ParameterType for OrderByParameterType {
    fn name(&self) -> &String {
        &self.name
    }
}

impl ParameterType for PredicateParameterType {
    fn name(&self) -> &String {
        &self.name
    }
}

pub const PRIMITIVE_ORDERING_OPTIONS: [&str; 2] = ["ASC", "DESC"];

impl TypeDefinitionProvider for OrderByParameterType {
    fn type_definition(&self, _system: &ModelSystem) -> SchemaTypeDefinition {
        match &self.kind {
            OrderByParameterTypeKind::Composite { parameters } => {
                let fields = parameters
                    .iter()
                    .map(|parameter| parameter.input_value())
                    .collect();

                SchemaTypeDefinition {
                    extend: false,
                    description: None,
                    name: Name::new(self.name()),
                    kind: SchemaTypeKind::InputObject(SchemaInputObjectType { fields }),
                }
            }
            OrderByParameterTypeKind::Primitive => SchemaTypeDefinition {
                extend: false,
                description: None,
                name: Name::new(self.name()),
                kind: SchemaTypeKind::Enum(SchemaEnumType {
                    values: PRIMITIVE_ORDERING_OPTIONS
                        .iter()
                        .map(|value| SchemaEnumValueDefinition {
                            description: None,
                            value: Name::new(value),
                        })
                        .collect(),
                }),
            },
        }
    }
}

// TODO: Reduce duplication from the above impl
impl TypeDefinitionProvider for PredicateParameterType {
    fn type_definition(&self, _system: &ModelSystem) -> SchemaTypeDefinition {
        match &self.kind {
            PredicateParameterTypeKind::Operator(parameters) => {
                let fields = parameters
                    .iter()
                    .map(|parameter| parameter.input_value())
                    .collect();

                SchemaTypeDefinition {
                    extend: false,
                    description: None,
                    name: Name::new(self.name()),
                    kind: SchemaTypeKind::InputObject(SchemaInputObjectType { fields }),
                }
            }
            PredicateParameterTypeKind::Composite {
                field_params,
                logical_op_params,
            } => {
                let parameters = [field_params, &logical_op_params[..]].concat();

                let fields = parameters
                    .iter()
                    .map(|parameter| parameter.input_value())
                    .collect();

                SchemaTypeDefinition {
                    extend: false,
                    description: None,
                    name: Name::new(self.name()),
                    kind: SchemaTypeKind::InputObject(SchemaInputObjectType { fields }),
                }
            }
            PredicateParameterTypeKind::ImplicitEqual => SchemaTypeDefinition {
                extend: false,
                description: None,
                name: Name::new(self.name()),
                kind: SchemaTypeKind::Scalar,
            },
        }
    }
}

// TODO: Reduce duplication from the above impl
impl TypeDefinitionProvider for ArgumentParameterType {
    fn type_definition(&self, system: &ModelSystem) -> SchemaTypeDefinition {
        let type_def = system
            .types
            .get(self.actual_type_id.unwrap())
            .unwrap()
            .type_definition(system);

        let kind = match type_def.fields() {
            Some(fields) => SchemaTypeKind::InputObject(SchemaInputObjectType {
                fields: fields
                    .iter()
                    .map(|field_definition| SchemaInputValueDefinition {
                        description: field_definition.description.clone(),
                        name: field_definition.name.clone(),
                        ty: field_definition.ty.clone(),
                        default_value: None,
                    })
                    .collect(),
            }),
            None => SchemaTypeKind::Scalar,
        };

        SchemaTypeDefinition {
            extend: false,
            name: Name::new(&self.name),
            description: None,
            kind,
        }
    }
}

impl TypeDefinitionProvider for LimitParameter {
    fn type_definition(&self, _system: &ModelSystem) -> SchemaTypeDefinition {
        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new(&self.name),
            kind: SchemaTypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider for OffsetParameter {
    fn type_definition(&self, _system: &ModelSystem) -> SchemaTypeDefinition {
        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new(&self.name),
            kind: SchemaTypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider for ArgumentParameter {
    fn type_definition(&self, _system: &ModelSystem) -> SchemaTypeDefinition {
        SchemaTypeDefinition {
            extend: false,
            description: None,
            name: Name::new(&self.name),
            kind: SchemaTypeKind::Scalar,
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::model::operation::*;
//     use crate::model::types::*;
//     use graphql_parser::parse_schema;

//     #[test]
//     fn scalar() {
//         let tpe = ParameterType {
//             name: "Int".to_owned(),
//             kind: ParameterTypeKind::Primitive,
//         };
//         let td = tpe.type_definition();
//         let expected = parse_schema::<&str>("scalar Int").unwrap();
//         assert_eq!(format!("{}", expected), format!("{}", td));
//     }

//     #[test]
//     fn complex() {
//         let id_param = Parameter {
//             name: "id".to_string(),
//             type_name: "Int".to_string(),
//             type_modifier: ModelTypeModifier::NonNull,
//             role: ParameterRole::Data,
//         };

//         let title_param = Parameter {
//             name: "title".to_string(),
//             type_name: "String".to_string(),
//             type_modifier: ModelTypeModifier::Optional,
//             role: ParameterRole::Data,
//         };

//         let parameters = vec![id_param, title_param];

//         let venue_parameter_type = ParameterType {
//             name: "Venue".to_string(),
//             kind: ParameterTypeKind::Composite { parameters },
//         };

//         let td = venue_parameter_type.type_definition();
//         let expected = parse_schema::<&str>(
//             "input Venue {
//             id: Int!,
//             title: String
//         }",
//         )
//         .unwrap();
//         assert_eq!(format!("{}", expected), format!("{}", td));
//     }
// }
