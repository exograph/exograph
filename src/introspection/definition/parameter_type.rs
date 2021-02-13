use graphql_parser::{
    schema::{EnumType, EnumValue, InputObjectType, InputValue, ScalarType, TypeDefinition},
    Pos,
};

use crate::model::types::{ParameterType, ParameterTypeKind};

use super::provider::{InputValueProvider, TypeDefinitionProvider};

impl<'a> TypeDefinitionProvider for ParameterType {
    fn type_definition(&self) -> TypeDefinition<String> {
        match &self.kind {
            ParameterTypeKind::Primitive => {
                TypeDefinition::Scalar(ScalarType::new(self.name.clone()))
            }
            ParameterTypeKind::Composite { parameters } => {
                let fields: Vec<InputValue<String>> = parameters
                    .iter()
                    .map(|parameter| parameter.input_value())
                    .collect();

                TypeDefinition::InputObject(InputObjectType {
                    position: Pos::default(),
                    description: None,
                    name: self.name.clone(),
                    directives: vec![],
                    fields: fields,
                })
            }
            ParameterTypeKind::Enum { values } => TypeDefinition::Enum(EnumType {
                position: Pos::default(),
                description: None,
                name: self.name.clone(),
                directives: vec![],
                values: values
                    .iter()
                    .map(|value| EnumValue::new(value.to_owned()))
                    .collect(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::{ModelTypeModifier, Parameter};
    use graphql_parser::parse_schema;
    use std::sync::Arc;

    #[test]
    fn scalar() {
        let tpe = ParameterType {
            name: "Int".to_owned(),
            kind: ParameterTypeKind::Primitive,
        };
        let td = tpe.type_definition();
        let expected = parse_schema::<&str>("scalar Int").unwrap();
        assert_eq!(format!("{}", expected), format!("{}", td));
    }

    #[test]
    fn complex() {
        let id_param = Parameter {
            name: "id".to_string(),
            tpe: Arc::new(ParameterType {
                name: "Int".to_string(),
                kind: ParameterTypeKind::Primitive,
            }),
            type_modifier: ModelTypeModifier::NonNull,
        };

        let title_param = Parameter {
            name: "title".to_string(),
            tpe: Arc::new(ParameterType {
                name: "String".to_string(),
                kind: ParameterTypeKind::Primitive,
            }),
            type_modifier: ModelTypeModifier::Optional,
        };

        let parameters = vec![id_param, title_param];

        let venue_parameter_type = ParameterType {
            name: "Venue".to_string(),
            kind: ParameterTypeKind::Composite { parameters },
        };

        let td = venue_parameter_type.type_definition();
        let expected = parse_schema::<&str>(
            "input Venue {
            id: Int!,
            title: String
        }",
        )
        .unwrap();
        assert_eq!(format!("{}", expected), format!("{}", td));
    }
}
