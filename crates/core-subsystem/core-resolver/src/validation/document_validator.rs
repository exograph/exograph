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
    normal_query_depth_limit: usize,
    introspection_query_depth_limit: usize,
}

impl<'a> DocumentValidator<'a> {
    pub fn new(
        schema: &'a Schema,
        operation_name: Option<String>,
        variables: Option<Map<String, Value>>,
        normal_query_depth_limit: usize,
        introspection_query_depth_limit: usize,
    ) -> Self {
        Self {
            schema,
            operation_name,
            variables,
            normal_query_depth_limit,
            introspection_query_depth_limit,
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
            self.normal_query_depth_limit,
            self.introspection_query_depth_limit,
        );

        operation_validator.validate(raw_operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::env;

    use async_graphql_parser::parse_query;
    use core_plugin_shared::{
        serializable_system::SerializableSystem, system_serializer::SystemSerializer,
    };

    #[test]
    fn argument_valid() {
        let schema = create_test_schema();

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, Some(variables), 10, 10);

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
        let validator = DocumentValidator::new(&schema, None, Some(variables), 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, Some("concert1".to_string()), None, 10, 10);

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));

        let validator = DocumentValidator::new(&schema, Some("concert2".to_string()), None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

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

        let validator = DocumentValidator::new(&schema, Some("foo".to_string()), None, 10, 10);

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn fragment_recursion_direct() {
        let schema = create_test_schema();

        let query = r#"
            query {
                concerts {
                    ...concertFields
                }
            }

            fragment concertFields on Concert {
                ...concertFields
            }
        "#;

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn fragment_recursion_indirect() {
        let schema = create_test_schema();

        let query = r#"
            query {
                concerts {
                    ...concertInfo
                }
            }

            fragment concertInfo on Concert {
                ...concertDetails
            }

            fragment concertDetails on Concert {
                ...concertInfo
            }
        "#;

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn query_depth_limit_direct() {
        let schema = create_test_schema();

        let query = r#"
            query {
                concerts { # 1
                    venue { # 2
                        concerts { # 3
                            venue { # 4
                                concerts { # 5
                                    id # 6
                                }
                            }
                        }
                    }
                }
            }

        "#;

        // valid
        let validator = DocumentValidator::new(&schema, None, None, 6, usize::MAX);
        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));

        // invalid: one level too deep
        let validator = DocumentValidator::new(&schema, None, None, 5, usize::MAX);
        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn query_depth_limit_through_fragment() {
        let schema = create_test_schema();

        let query = r#"
            query {
                concerts { # 1
                    venue { # 2
                        concerts { # 3
                            ...venueInfo
                        }
                    }
                }
            }
            fragment venueInfo on Venue {
                venue { # 4
                    concerts { # 5
                        id # 6
                    }
                }
            }
        "#;

        // valid
        let validator = DocumentValidator::new(&schema, None, None, 6, usize::MAX);
        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));

        // invalid: one level too deep
        let validator = DocumentValidator::new(&schema, None, None, 5, usize::MAX);
        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    #[test]
    fn introspection_query_depth_limit_direct() {
        let schema = create_test_schema();

        let query = r#"
            query {
                __schema { # 1
                    types { # 2
                        name # 3
                    }
                }
            }

        "#;

        // valid
        let validator = DocumentValidator::new(&schema, None, None, usize::MAX, 3);
        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));

        // invalid: one level too deep
        let validator = DocumentValidator::new(&schema, None, None, usize::MAX, 2);
        insta::assert_debug_snapshot!(validator.validate(create_query_document(query)));
    }

    fn create_variables(variables: &str) -> Map<String, Value> {
        serde_json::from_str(variables).unwrap()
    }

    fn create_test_schema() -> Schema {
        let test_exo = r#"
            @postgres
            module LogModule {
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
        let postgres_subsystem = create_postgres_system_from_str(test_exo, "test.exo".to_string());

        Schema::new(
            postgres_subsystem.schema_types(),
            postgres_subsystem.schema_queries(),
            postgres_subsystem.schema_mutations(),
        )
    }

    fn create_query_document(query_str: &str) -> ExecutableDocument {
        parse_query(query_str).unwrap()
    }

    /// Creates a postgres subsystem from a exo file for test purposes. This creates a soft
    /// dependency on the postgres subsystem (its shared library must be build before executing the
    /// tests).
    ///
    /// Note: This arrangement is not ideal, since this make rust-analyzer think we have two cyclic
    /// dependencies due to `dev-dependencies`: `this crate -> (core_plugin_interface) -> this
    /// crate` and `this crate -> builder -> core_plugin_interface -> this crate`. However, this is
    /// a rust-analyzer bug (https://github.com/rust-lang/rust-analyzer/issues/14167). An
    /// alternative would be to create a separate crate for these tests, but that would be overkill
    /// for now and will make these test live outside of the core being tested. A more dramatic
    /// solution would be to create a "test" subsystem that can be used for testing
    /// purposes--something to consider in the future.
    pub fn create_postgres_system_from_str(
        model_str: &str,
        file_name: String,
    ) -> Box<dyn core_plugin_interface::core_resolver::plugin::SubsystemResolver> {
        let serialized_system = builder::build_system_from_str(model_str, file_name).unwrap();
        let system = SerializableSystem::deserialize(serialized_system).unwrap();

        let subsystem_id = "postgres";
        let subsystem_library_name = format!("{subsystem_id}_resolver_dynamic");
        let loader =
            core_plugin_interface::interface::load_subsystem_loader(&subsystem_library_name)
                .unwrap();

        let subsystem = system
            .subsystems
            .into_iter()
            .find(|subsystem| subsystem.id == subsystem_id)
            .unwrap()
            .serialized_subsystem;

        // Set the EXO_POSTGRES_URL to avoid failure of `loader::init` later. Since we are not actually
        // connecting to a database, this is fine (we are only interested get queries, mutations, and types
        // to build the schema)
        env::set_var(
            "EXO_POSTGRES_URL",
            "postgres://postgres:postgres@localhost:5432/exo_test",
        );

        loader.init(subsystem).unwrap()
    }
}
