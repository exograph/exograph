use async_graphql_parser::types::{DocumentOperations, ExecutableDocument};
use async_graphql_value::Name;
use payas_model::model::system::ModelSystem;
use serde_json::{Map, Value};
use tracing::instrument;

use crate::graphql::validation::validation_error::ValidationError;

use super::{operation::ValidatedOperation, operation_validator::OperationValidator};

/// Context for validating a document.
pub struct DocumentValidator<'a> {
    system: &'a ModelSystem,
    operation_name: Option<String>,
    variables: Option<Map<String, Value>>,
}

impl<'a> DocumentValidator<'a> {
    pub fn new(
        system: &'a ModelSystem,
        operation_name: Option<String>,
        variables: Option<Map<String, Value>>,
    ) -> Self {
        Self {
            system,
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

    #[test]
    fn no_arguments_valid() {
        let system = create_test_system();

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        let query = r#"
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
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn argument_valid() {
        let system = create_test_system();

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        let query = r#"
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
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn with_operation_name_valid() {
        let system = create_test_system();

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        let query = r#"
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
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn stray_argument_invalid() {
        let system = create_test_system();

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        let query = r#"
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
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn unspecified_required_argument_invalid() {
        let system = create_test_system();

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        let query = r#"
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
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn variable_resolution_valid() {
        let system = create_test_system();

        let variables = create_variables(
            r#"
            {
                "concert_id": 1,
                "venue_id": 2
            }"#,
        );

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: Some(variables),
        };

        let query = r#"
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
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn variable_resolution_invalid() {
        let system = create_test_system();

        let variables = create_variables(r#"{ "concert_id": 2 }"#);
        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: Some(variables),
        };

        let query = r#"
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
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn invalid_subfield() {
        let system = create_test_system();

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
                concert(id: 1) {
                    id
                    title
                    foobar
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn aliases_valid() {
        let system = create_test_system();

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
                firstConcert: concert(id: 1) {
                    id
                    headLine: title
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn multi_operations_valid() {
        let system = create_test_system();

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

        let validator = DocumentValidator {
            system: &system,
            operation_name: Some("concert1".to_string()),
            variables: None,
        };

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));

        let validator = DocumentValidator {
            system: &system,
            operation_name: Some("concert2".to_string()),
            variables: None,
        };

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn multi_operations_no_operation_name_invalid() {
        let system = create_test_system();

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

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn multi_operations_mismatched_operation_name_invalid() {
        let system = create_test_system();

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

        let validator = DocumentValidator {
            system: &system,
            operation_name: Some("foo".to_string()),
            variables: None,
        };

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
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
        "#;
        payas_parser::build_system_from_str(test_clay, "test.clay".to_string()).unwrap()
    }

    #[test]
    fn basic_mutation() {
        let system = create_test_system();

        let validator = DocumentValidator {
            system: &system,
            operation_name: None,
            variables: None,
        };

        let mutation = r#"
            mutation {
                createConcert(data: {title: "Concert 1", venue: {id: 1}}) {
                    id
                    title
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(mutation)));
    }

    fn create_query_document(query_str: &str) -> ExecutableDocument {
        parse_query(query_str).unwrap()
    }
}
