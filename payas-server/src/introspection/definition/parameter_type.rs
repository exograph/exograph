use async_graphql_parser::{
    types::{
        EnumType, EnumValueDefinition, InputObjectType, InputValueDefinition, TypeDefinition,
        TypeKind,
    },
    Pos, Positioned,
};
use async_graphql_value::Name;

use crate::introspection::{definition::type_introspection::TypeDefinitionIntrospection, util::*};
use payas_model::model::{
    argument::{ArgumentParameter, ArgumentParameterType},
    limit_offset::{LimitParameter, OffsetParameter},
    order::*,
    predicate::*,
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
            PredicateParameterTypeKind::Operator(parameters) => {
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
            PredicateParameterTypeKind::Composite(parameters, boolean_params) => {
                let parameters = [parameters, &boolean_params[..]].concat();

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

// TODO: Reduce duplication from the above impl
impl TypeDefinitionProvider for ArgumentParameterType {
    fn type_definition(&self, system: &ModelSystem) -> TypeDefinition {
        let type_def = system
            .types
            .get(self.actual_type_id.unwrap())
            .unwrap()
            .type_definition(system);

        let kind = match type_def.fields() {
            Some(fields) => TypeKind::InputObject(InputObjectType {
                fields: fields
                    .iter()
                    .map(|positioned| {
                        let field_definition = &positioned.node;

                        Positioned {
                            pos: positioned.pos,
                            node: InputValueDefinition {
                                description: field_definition.description.clone(),
                                name: field_definition.name.clone(),
                                ty: field_definition.ty.clone(),
                                default_value: None,
                                directives: vec![],
                            },
                        }
                    })
                    .collect(),
            }),
            None => TypeKind::Scalar,
        };

        TypeDefinition {
            extend: false,
            name: default_positioned_name(&self.name),
            description: None,
            directives: vec![],
            kind,
        }
    }
}

impl TypeDefinitionProvider for LimitParameter {
    fn type_definition(&self, _system: &ModelSystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider for OffsetParameter {
    fn type_definition(&self, _system: &ModelSystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
        }
    }
}

impl TypeDefinitionProvider for ArgumentParameter {
    fn type_definition(&self, _system: &ModelSystem) -> TypeDefinition {
        TypeDefinition {
            extend: false,
            description: None,
            name: default_positioned_name(&self.name),
            directives: vec![],
            kind: TypeKind::Scalar,
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
