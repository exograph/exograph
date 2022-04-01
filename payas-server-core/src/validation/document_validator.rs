use async_graphql_parser::types::ExecutableDocument;
use serde_json::{Map, Value};

use crate::{error::ExecutionError, introspection::schema::Schema};

use super::{document::ValidatedDocument, operation_validator::OperationValidator};

/// Context for validating a document.
pub struct DocumentValidator<'a> {
    schema: &'a Schema,
    operation_name: Option<String>,
    variables: Option<Map<String, Value>>,
}

impl<'a> DocumentValidator<'a> {
    pub fn new(
        schema: &'a Schema,
        operation_name: Option<String>,
        variables: Option<Map<String, Value>>,
    ) -> Self {
        Self {
            schema,
            operation_name,
            variables,
        }
    }

    /// Validate the query payload.
    /// Validations performed:
    /// - Validate that all operations are of the same type (queries or mutations)
    /// - Validate that there is at least one operation
    /// - Other validations are delegated to the operation validator
    pub fn validate(
        self,
        document: ExecutableDocument,
    ) -> Result<ValidatedDocument, ExecutionError> {
        let operation_validator = OperationValidator::new(
            self.schema,
            self.operation_name,
            self.variables,
            document.fragments,
        );

        let operations = document
            .operations
            .iter()
            .map(|operation| operation_validator.validate_operation(operation))
            .collect::<Result<Vec<_>, _>>()?;

        let operation_typ = match &operations.first() {
            Some(operation) => {
                let same_operation_type = operations.iter().all(|op| op.typ == operation.typ);
                if same_operation_type {
                    Ok(operation.typ)
                } else {
                    Err(ExecutionError::DifferentOperationTypes)
                }
            }
            None => Err(ExecutionError::NoOperationFound),
        }?;

        Ok(ValidatedDocument {
            operations,
            operation_typ,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql_parser::parse_query;

    #[test]
    fn argument_valid() {
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
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
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
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
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
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
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
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
        let schema = create_test_schema();

        let variables = create_variables(
            r#"
            {
                "concert_id": 1,
                "venue_id": 2
            }"#,
        );

        let validator = DocumentValidator {
            schema: &schema,
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
        let schema = create_test_schema();

        let variables = create_variables(r#"{ "concert_id": 2 }"#);
        let validator = DocumentValidator {
            schema: &schema,
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
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
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
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
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

    fn create_variables(variables: &str) -> Map<String, Value> {
        serde_json::from_str(variables).unwrap()
    }

    fn create_test_schema() -> Schema {
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
        let system =
            payas_parser::build_system_from_str(test_clay, "test.clay".to_string()).unwrap();
        Schema::new(&system)
    }

    fn create_query_document(query_str: &str) -> ExecutableDocument {
        parse_query(query_str).unwrap()
    }
}
