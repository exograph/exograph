use graphql_parser::{Pos, schema::{EnumType, EnumValue, InputObjectType, InputValue, ScalarType, TypeDefinition}};

use crate::model::{order::*, predicate::*};

use super::{parameter::Parameter, provider::{*, TypeDefinitionProvider}};

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

impl<'a> TypeDefinitionProvider for OrderByParameterType {
    fn type_definition(&self) -> TypeDefinition<String> {
        match &self.kind {
            OrderByParameterTypeKind::Composite { parameters } => {
                let fields: Vec<InputValue<String>> = parameters
                    .iter()
                    .map(|parameter| parameter.input_value())
                    .collect();

                TypeDefinition::InputObject(InputObjectType {
                    position: Pos::default(),
                    description: None,
                    name: self.name().clone(),
                    directives: vec![],
                    fields: fields,
                })
            }
            OrderByParameterTypeKind::Enum { values } => TypeDefinition::Enum(EnumType {
                position: Pos::default(),
                description: None,
                name: self.name().clone(),
                directives: vec![],
                values: values
                    .iter()
                    .map(|value| EnumValue::new(value.to_owned()))
                    .collect(),
            }),
        }
    }
}

// TODO: Reduce duplication from the above impl
impl<'a> TypeDefinitionProvider for PredicateParameterType {
    fn type_definition(&self) -> TypeDefinition<String> {
        match &self.kind {
            PredicateParameterTypeKind::Composite { parameters } => {
                let fields: Vec<InputValue<String>> = parameters
                    .iter()
                    .map(|parameter| parameter.input_value())
                    .collect();

                TypeDefinition::InputObject(InputObjectType {
                    position: Pos::default(),
                    description: None,
                    name: self.name().clone(),
                    directives: vec![],
                    fields: fields,
                })
            }
            PredicateParameterTypeKind::Primitive => TypeDefinition::Scalar(ScalarType::new(self.name.clone())),
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
