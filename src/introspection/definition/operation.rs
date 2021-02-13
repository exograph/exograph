use graphql_parser::{
    schema::{Field, InputValue},
    Pos,
};

use super::provider::{FieldDefinitionProvider, InputValueProvider};
use crate::{introspection::util, model::types::*};

// Field defintion for the query such as `venue(id: Int!): Venue`, combining such fields will form
// the Query, Mutation, and Subscription object defintion
impl<'a> FieldDefinitionProvider<'a> for Operation {
    fn field_definition(&self) -> Field<'a, String> {
        let fields: Vec<InputValue<'a, String>> = self
            .parameters
            .iter()
            .map(|parameter| parameter.input_value())
            .collect();

        Field {
            position: Pos::default(),
            description: None,
            name: self.name.clone(),
            arguments: fields,
            field_type: util::value_type(
                &self.return_type.model_type.name,
                &self.return_type.model_type_modifier,
            ),
            directives: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::test_util::common_test_data::*;
    use std::sync::Arc;

    #[test]
    fn simple_operation() {
        let venue = venue_model_type();

        let id_param = Parameter {
            name: "id".to_string(),
            tpe: Arc::new(ParameterType {
                name: "Int".to_string(),
                kind: ParameterTypeKind::Primitive,
            }),
            type_modifier: ModelTypeModifier::NonNull,
        };

        let return_type = OperationReturnType {
            model_type: venue,
            model_type_modifier: ModelTypeModifier::NonNull,
        };

        let op = Operation {
            name: "venue".to_string(),
            parameters: vec![id_param],
            return_type: return_type,
        };

        assert_eq!(
            "venue(id: Int!): Venue!\n",
            format!("{}", op.field_definition())
        );
    }
}
