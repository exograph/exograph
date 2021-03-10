use crate::introspection::definition::parameter::Parameter;
use graphql_parser::{
    schema::{Field, InputValue},
    Pos,
};

use super::provider::{FieldDefinitionProvider, InputValueProvider};
use crate::{introspection::util, model::operation::*};

pub trait Operation {
    fn name(&self) -> &String;
    fn parameters(&self) -> Vec<&dyn Parameter>;
    fn return_type(&self) -> &OperationReturnType;
}

impl Operation for Query {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        let mut params: Vec<&dyn Parameter> = vec![];
        match &self.predicate_parameter {
            Some(param) => params.push(param),
            None => {}
        }
        match &self.order_by_param {
            Some(param) => params.push(param),
            None => {}
        }

        params
    }

    fn return_type(&self) -> &OperationReturnType {
        &self.return_type
    }
}

// Field defintion for the query such as `venue(id: Int!): Venue`, combining such fields will form
// the Query, Mutation, and Subscription object defintion
impl<'a, T: Operation> FieldDefinitionProvider<'a> for T {
    fn field_definition(&self) -> Field<'a, String> {
        let fields: Vec<InputValue<'a, String>> = self
            .parameters()
            .iter()
            .map(|parameter| parameter.input_value())
            .collect();

        Field {
            position: Pos::default(),
            description: None,
            name: self.name().clone(),
            arguments: fields,
            field_type: util::value_type(
                &self.return_type().type_name,
                &self.return_type().type_modifier,
            ),
            directives: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::predicate::*;
    use crate::model::test_util::common_test_data::*;
    use crate::model::types::*;

    #[test]
    fn simple_operation() {
        let system = test_system();
        let venue = system.find_type("Venue").unwrap();

        let id_param = PredicateParameter {
            name: "id".to_string(),
            type_name: "Int".to_string(),
            type_modifier: ModelTypeModifier::NonNull,
        };

        let return_type = OperationReturnType {
            type_name: venue.name.clone(),
            type_modifier: ModelTypeModifier::NonNull,
        };

        let op = Query {
            name: "venue".to_string(),
            predicate_parameter: Some(id_param),
            order_by_param: None,
            return_type: return_type,
        };

        assert_eq!(
            "venue(id: Int!): Venue!\n",
            format!("{}", op.field_definition())
        );
    }
}
