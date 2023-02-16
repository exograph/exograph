use crate::validation::validation_error::ValidationError;
use async_graphql_parser::types::{DocumentOperations, ExecutableDocument};
use async_graphql_value::Name;
use serde_json::{Map, Value};
use tracing::instrument;

use crate::introspection::definition::schema::Schema;

use super::{operation::ValidatedOperation, operation_validator::OperationValidator};

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
    use core_plugin_shared::{
        serializable_system::SerializableSystem, system_serializer::SystemSerializer,
    };

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

    #[test]
    fn mergeable_leaf_fields() {
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
               concerts {
                    title
                    id
                    title
                    title
                    t: title # This should not be merged with the previous other title fields
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn mergeable_leaf_fields_with_alias() {
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
               concerts {
                    t: title
                    id
                    t: title
                    t: title
                    tt: title # All t's should be merged, but not tt (even if it points to the same field)
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn unmergeable_leaf_fields_all_aliases() {
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
               concerts {
                    t: title # The `t` alias points to a different field than the other `t` aliases
                    id
                    t: id
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn unmergeable_leaf_fields_mixed_aliases() {
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
               concerts {
                    id: title # The `t` alias points to a different field than the next field
                    id
                    t: id
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn mergeable_non_leaf_fields() {
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
               concerts {
                    id
                    venue {
                        id
                        name
                    }
                    venue {
                        name
                        published
                    }
                    v: venue {
                        name
                        published
                    }
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn unmergeable_non_leaf_fields() {
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
               concerts {
                    id
                    venue {
                        id
                        n: name # The alias 'n' points to a different field than the other 'n' aliases (in the other 'venue' fields)
                    }
                    venue {
                        name
                        n: published
                    }
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn mergeable_non_leaf_fields_with_alias() {
        let schema = create_test_schema();

        let validator = DocumentValidator {
            schema: &schema,
            operation_name: None,
            variables: None,
        };

        let query = r#"
            query {
               concerts {
                    id
                    v: venue {
                        id
                        name
                    }
                    v: venue {
                        name
                        published
                    }
                    vv: venue {
                        name
                        published
                    }
                }
            }
        "#;

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn multi_operations_valid() {
        let schema = create_test_schema();

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
            schema: &schema,
            operation_name: Some("concert1".to_string()),
            variables: None,
        };

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));

        let validator = DocumentValidator {
            schema: &schema,
            operation_name: Some("concert2".to_string()),
            variables: None,
        };

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn multi_operations_no_operation_name_invalid() {
        let schema = create_test_schema();

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
            schema: &schema,
            operation_name: None,
            variables: None,
        };

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn multi_operations_mismatched_operation_name_invalid() {
        let schema = create_test_schema();

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
            schema: &schema,
            operation_name: Some("foo".to_string()),
            variables: None,
        };

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    fn create_variables(variables: &str) -> Map<String, Value> {
        serde_json::from_str(variables).unwrap()
    }

    fn create_test_schema() -> Schema {
        let test_clay = r#"
            @postgres
            service LogService {
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }

                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    published: Boolean
                    concerts: Set<Concert>
                }
            }
        "#;
        let postgres_subsystem =
            create_postgres_system_from_str(test_clay, "test.clay".to_string());

        Schema::new(
            postgres_subsystem.schema_types(),
            postgres_subsystem.schema_queries(),
            postgres_subsystem.schema_mutations(),
        )
    }

    fn create_query_document(query_str: &str) -> ExecutableDocument {
        parse_query(query_str).unwrap()
    }

    // TODO: Rethink this approach, where core (even though it's a test) depends on a specific subsystem (database)

    use postgres_model::subsystem::PostgresSubsystem;
    pub fn create_postgres_system_from_str(
        model_str: &str,
        file_name: String,
    ) -> PostgresSubsystem {
        let serialized_system = builder::build_system_from_str(model_str, file_name).unwrap();
        let system = SerializableSystem::deserialize(serialized_system).unwrap();

        deserialize_postgres_subsystem(system)
    }

    fn deserialize_postgres_subsystem(system: SerializableSystem) -> PostgresSubsystem {
        system
            .subsystems
            .into_iter()
            .find_map(|subsystem| {
                if subsystem.id == "postgres" {
                    Some(PostgresSubsystem::deserialize(
                        subsystem.serialized_subsystem,
                    ))
                } else {
                    None
                }
            })
            // If there is no database subsystem in the serialized system, create an empty one
            .unwrap_or_else(|| Ok(PostgresSubsystem::default()))
            .unwrap()
    }
}
