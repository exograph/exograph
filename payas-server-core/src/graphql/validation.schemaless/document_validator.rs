use async_graphql_parser::types::{DocumentOperations, ExecutableDocument};
use async_graphql_value::Name;
use payas_model::model::system::ModelSystem;
use serde_json::{Map, Value};
use tracing::instrument;

use crate::graphql::{
    introspection::definition::schema::Schema, validation::validation_error::ValidationError,
};

use super::{operation::ValidatedOperation, operation_validator::OperationValidator};

/// Context for validating a document.
pub struct DocumentValidator<'a> {
    system: &'a ModelSystem,
    schema: &'a Schema,
    operation_name: Option<String>,
    variables: Option<Map<String, Value>>,
}

impl<'a> DocumentValidator<'a> {
    pub fn new(
        system: &'a ModelSystem,
        schema: &'a Schema,
        operation_name: Option<String>,
        variables: Option<Map<String, Value>>,
    ) -> Self {
        Self {
            system,
            schema,
            operation_name,
            variables,
        }
    }

    /// Validate the query payload.
    ///
    /// Validations performed:
    /// - Validate that either there is only one operation or the operation name specified matches one of the operations in the document
    /// - Validate that there is at least one operation
    /// - Other validations are delegated to the operation validator
    #[instrument(
        name = "DocumentValidator::validate"
        skip(self, document)
        )]
    pub fn validate(
        self,
        document: ExecutableDocument,
    ) -> Result<ValidatedOperation, ValidationError> {
        let (operation_name, raw_operation) = match document.operations {
            DocumentOperations::Single(operation) => Ok((self.operation_name, operation)),
            DocumentOperations::Multiple(mut operations) => {
                if operations.is_empty() {
                    Err(ValidationError::NoOperationFound)
                } else {
                    match self.operation_name {
                        None if operations.len() == 1 => {
                            // Per https://graphql.org/learn/queries/#operation-name, `operationName` is required
                            // only for multiple operations, but async-graphql parses a named operation (`query Foo { ... }`)
                            // to `DocumentOperations::Multiple` even if there is only one operation. So we add an additional
                            // check here to make sure that the operation name is enforced only for truly multiple operations.

                            // This unwrap is okay because we already check that there is exactly one operation.
                            let (operation_name, operation) =
                                operations.into_iter().next().unwrap();
                            Ok((Some(operation_name.to_string()), operation))
                        }
                        None => Err(ValidationError::MultipleOperationsNoOperationName),
                        Some(operation_name) => {
                            let operation = operations.remove(&Name::new(&operation_name));

                            match operation {
                                None => {
                                    Err(ValidationError::MultipleOperationsUnmatchedOperationName(
                                        operation_name,
                                    ))
                                }
                                Some(operation) => Ok((Some(operation_name), operation)),
                            }
                        }
                    }
                }
            }
        }?;

        let operation_validator = OperationValidator::new(
            self.system,
            self.schema,
            operation_name,
            self.variables,
            document.fragments,
        );

        operation_validator.validate(raw_operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql_parser::parse_query;

    // Macro to avoid insta-related issues with the test names when asserted through a helper function
    // It accepts arguments in the same way as that would be passed to the graphql endpoint
    macro_rules! assert_validation {
        ($query:expr, $operation_name:expr, $variables:expr) => {
            let system = create_test_system();
            let schema = Schema::new(&system);
            let variables = if $variables.is_empty() {
                None
            } else {
                Some(create_variables($variables))
            };

            let operation_name = if $operation_name.is_empty() {
                None
            } else {
                Some($operation_name.to_string())
            };

            let validator = DocumentValidator {
                system: &system,
                schema: &schema,
                operation_name,
                variables,
            };

            insta::assert_debug_snapshot!(validator.validate(create_query_document($query)));
        };
    }

    #[test]
    fn no_arguments_valid() {
        assert_validation!(
            r#"
                query {
                    concerts {
                        id
                        title
                        venue {
                            id
                            name
                        }
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn primitive_argument_valid() {
        assert_validation!(
            r#"
                query {
                    concert(id: 1) {
                        id
                        title
                        venue {
                            id
                            name
                        }
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn composite_argument_valid() {
        assert_validation!(
            r#"
                query {
                    concerts(where: {id: {eq: 1}}) {
                        id
                        title
                        venue {
                            id
                            name
                        }
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn composite_list_argument_valid() {
        assert_validation!(
            r#"
                query {
                    concerts(where: {and: [{id: {gt: 1}}, {id: {lt: 3}}]}) {
                        id
                        title
                        venue {
                            id
                            name
                        }
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn composite_order_by_valid_implicit_array() {
        assert_validation!(
            r#"
                query {
                    concerts(orderBy: {title: DESC}) {
                        id
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn composite_order_by_valid_explicit_array() {
        assert_validation!(
            r#"
                query {
                    concerts(orderBy: [{id: ASC}, {title: DESC}]) {
                        id
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn composite_order_by_invalid_overly_nested_array() {
        assert_validation!(
            r#"
                query {
                    concerts(orderBy: [[{id: ASC}, {title: DESC}]]) {
                        id
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn with_operation_name_valid() {
        assert_validation!(
            r#"
                query ConcertById {
                    concert(id: 1) {
                        id
                        title
                        venue {
                            id
                            name
                        }
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn stray_argument_invalid() {
        assert_validation!(
            r#"
                query {
                    concert(id: 1, foo: "bar") { # foo is not a valid argument
                        id
                        title
                        venue {
                            id
                            name
                        }
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn unspecified_required_argument_invalid() {
        assert_validation!(
            r#"
                query {
                    concert { # id argument is required here
                        id
                        title
                        venue {
                            id
                            name
                        }
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn variable_resolution_valid() {
        assert_validation!(
            r#"
                query($concert_id: Int!, $venue_id: Int!) {
                    concert(id: $concert_id) { # id argument is required here
                        id
                        title
                    }
                    venue(id: $venue_id) {
                        id
                        name
                    }
                }
            "#,
            "",
            r#"
            {
                "concert_id": 1,
                "venue_id": 2
            }"#
        );
    }

    #[test]
    fn variable_resolution_invalid() {
        assert_validation!(
            r#"
            query($concert_id: Int!, $venue_id: Int!) { # venue_id is not a specified in variables
                concert(id: $concert_id) { # id argument is required here
                    id
                    title
                }
                venue(id: $venue_id) {
                    id
                    name
                }
            }
        "#,
            "",
            r#"{ "concert_id": 2 }"#
        );
    }

    #[test]
    fn invalid_subfield() {
        assert_validation!(
            r#"
                query {
                    concert(id: 1) {
                        id
                        title
                        foobar
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn aliases_valid() {
        assert_validation!(
            r#"
                query {
                    firstConcert: concert(id: 1) {
                        id
                        headLine: title
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn multi_operations_valid() {
        let query = r#"
            query concert1 {
                concert(id: 1) {
                    id
                    headLine: title
                }
            }

            query concert2 {
                concert(id: 2) {
                    id
                    headLine: title
                }
            }
        "#;
        assert_validation!(query, "concert1", "");
        assert_validation!(query, "concert2", "");
    }

    #[test]
    fn multi_operations_no_operation_name_invalid() {
        assert_validation!(
            r#"
                query concert1 {
                    concert(id: 1) {
                        id
                        headLine: title
                    }
                }

                query concert2 {
                    concert(id: 2) {
                        id
                        headLine: title
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn multi_operations_mismatched_operation_name_invalid() {
        assert_validation!(
            r#"
                query concert1 {
                    concert(id: 1) {
                        id
                        headLine: title
                    }
                }

                query concert2 {
                    concert(id: 2) {
                        id
                        headLine: title
                    }
                }
            "#,
            "foo",
            ""
        );
    }

    #[test]
    fn basic_mutation() {
        assert_validation!(
            r#"
                mutation {
                    createConcert(data: {title: "Concert 1", venue: {id: 1}}) {
                        id
                        title
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn create_mutation_list_input() {
        assert_validation!(
            r#"
                mutation {
                    createConcerts(data: [{title: "Concert 1", venue: {id: 1}}, {title: "Concert 2", venue: {id: 2}}]) {
                        id
                        title
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn create_mutation_invalid_overly_nested_list_input() {
        assert_validation!(
            r#"
                mutation {
                    createConcerts(data: [[{title: "Concert 1", venue: {id: 1}}]]) {
                        id
                        title
                    }
                }
            "#,
            "",
            ""
        );
    }

    #[test]
    fn service_mutation() {
        assert_validation!(
            r#"
                mutation {
                    processInfo(info: { level: 1, message: "allowed" })
                }
            "#,
            "",
            ""
        );
    }

    fn create_variables(variables: &str) -> Map<String, Value> {
        serde_json::from_str(variables).unwrap()
    }

    fn create_test_system() -> ModelSystem {
        let test_clay = r#"
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                venue: Venue
            }

            model Venue {
                id: Int = autoincrement() @pk
                name: String
                concerts: Set<Concert>
            }

            @external("test.js")
            service TestService {
                input type Info {
                    level: Int
                    message: String
                }
            
                mutation processInfo(info: Info): Boolean
            }
        "#;
        payas_parser::build_system_from_str(test_clay, "test.clay".to_string()).unwrap()
    }

    fn create_query_document(query_str: &str) -> ExecutableDocument {
        parse_query(query_str).unwrap()
    }
}
