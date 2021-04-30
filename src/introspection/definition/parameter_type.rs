use async_graphql_parser::{
    types::{EnumType, EnumValueDefinition, InputObjectType, TypeDefinition, TypeKind},
    Pos, Positioned,
};
use async_graphql_value::Name;

use crate::model::{order::*, predicate::*};
use crate::{introspection::util::*, model::system::ModelSystem};

use super::{parameter::Parameter, provider::*};

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

pub enum ParameterTypeKind {
    Primitive,
    Composite { parameters: Vec<Box<dyn Parameter>> },
    Enum { values: Vec<String> },
}

pub const PRIMITIVE_ORDERING_OPTIONS: [&str; 2] = ["ASC", "DESC"];

impl TypeDefinitionProvider for OrderByParameterType {
    fn type_definition(&self, _system: &ModelSystem) -> TypeDefinition {
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
                name: Positioned::new(Name::new(self.name()), Pos::default()),
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

// TODO: Reduce duplication from the above impl
impl TypeDefinitionProvider for PredicateParameterType {
    fn type_definition(&self, _system: &ModelSystem) -> TypeDefinition {
        match &self.kind {
            PredicateParameterTypeKind::Opeartor(parameters)
            | PredicateParameterTypeKind::Composite(parameters) => {
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
            PredicateParameterTypeKind::ImplicitEqual => TypeDefinition {
                extend: false,
                description: None,
                name: default_positioned_name(self.name()),
                directives: vec![],
                kind: TypeKind::Scalar,
            },
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
