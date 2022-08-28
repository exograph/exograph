use crate::graphql::introspection::{
    definition::parameter::Parameter, schema::SchemaFieldDefinition,
};

use async_graphql_value::Name;
use payas_model::model::{
    operation::{
        DatabaseMutationKind, DatabaseQueryParameter, Mutation, MutationKind, OperationReturnType,
        Query, QueryKind,
    },
    system::ModelSystem,
};

use super::provider::{FieldDefinitionProvider, InputValueProvider};
use crate::graphql::introspection::util;

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

        macro_rules! populate_params (
            ($param_name:expr) => {
                match $param_name {
                    Some(param) => params.push(param),
                    None => {}
                }
            }
        );

        match &self.kind {
            QueryKind::Database(db_query_param) => {
                let DatabaseQueryParameter {
                    predicate_param,
                    order_by_param,
                    limit_param,
                    offset_param,
                } = db_query_param.as_ref();
                populate_params!(&predicate_param);
                populate_params!(&order_by_param);
                populate_params!(&limit_param);
                populate_params!(&offset_param);
            }
            QueryKind::Service { argument_param, .. } => {
                for arg in argument_param.iter() {
                    params.push(arg)
                }
            }
        }

        params
    }

    fn return_type(&self) -> &OperationReturnType {
        &self.return_type
    }
}

impl Operation for Mutation {
    fn name(&self) -> &String {
        &self.name
    }

    fn parameters(&self) -> Vec<&dyn Parameter> {
        match &self.kind {
            MutationKind::Database { kind } => match kind {
                DatabaseMutationKind::Create(data_param) => vec![data_param],
                DatabaseMutationKind::Delete(predicate_param) => vec![predicate_param],
                DatabaseMutationKind::Update {
                    data_param,
                    predicate_param,
                } => vec![predicate_param, data_param],
            },

            MutationKind::Service { argument_param, .. } => argument_param
                .iter()
                .map(|param| {
                    let param: &dyn Parameter = param;
                    param
                })
                .collect(),
        }
    }

    fn return_type(&self) -> &OperationReturnType {
        &self.return_type
    }
}

// Field definition for the query such as `venue(id: Int!): Venue`, combining such fields will form
// the Query, Mutation, and Subscription object definition
impl<T: Operation> FieldDefinitionProvider for T {
    fn field_definition(&self, _system: &ModelSystem) -> SchemaFieldDefinition {
        let fields = self
            .parameters()
            .iter()
            .map(|parameter| parameter.input_value())
            .collect();

        SchemaFieldDefinition {
            description: None,
            name: Name::new(self.name()),
            arguments: fields,
            ty: util::value_type(
                &self.return_type().type_name,
                &self.return_type().type_modifier,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::model::predicate::*;
    // use crate::model::test_util::common_test_data::*;
    // use crate::model::types::*;

    #[test]
    fn simple_operation() {
        // let system = test_system();
        // let venue = system.types.get_by_key("Venue").unwrap();

        // system.queries

        // let id_predicate_type_id = system.predicate_types.get_by_key("Int");

        // let id_param = PredicateParameter {
        //     name: "id".to_string(),
        //     type_name: "Int".to_string(),
        //     type_id: id_predicate_type_id,
        //     type_modifier: ModelTypeModifier::NonNull,
        // };

        // let return_type = OperationReturnType {
        //     type_name: venue.name.clone(),
        //     type_modifier: ModelTypeModifier::NonNull,
        // };

        // let op = Query {
        //     name: "venue".to_string(),
        //     predicate_parameter: Some(id_param),
        //     order_by_param: None,
        //     return_type: return_type,
        // };

        // assert_eq!(
        //     "venue(id: Int!): Venue!\n",
        //     format!("{}", op.field_definition())
        // );
    }
}
