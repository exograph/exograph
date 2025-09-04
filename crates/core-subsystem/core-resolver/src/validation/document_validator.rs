// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

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
    use std::sync::Arc;

    use super::*;

    use async_graphql_parser::parse_query;
    use core_model_builder::plugin::BuildMode;
    use exo_env::MapEnvironment;
    use exo_sql::DatabaseClientManager;

    macro_rules! assert_debug {
        ($src:expr, $fn_name:expr) => {
            insta::with_settings!({prepend_module_to_snapshot => false}, {
                #[cfg(target_family = "wasm")]
                {
                    let value = $src;
                    let expected = include_str!(concat!("./snapshots/", $fn_name, ".snap"));
                    let split_expected = expected.split("---\n").skip(2).collect::<Vec<&str>>().join("---");
                    let serialized = std::format!("{:#?}", value);
                    assert_eq!(split_expected, serialized + "\n");
                }

                #[cfg(not(target_family = "wasm"))]
                {

                    insta::assert_debug_snapshot!($src)
                }
            })
        };
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn argument_valid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "argument_valid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn with_operation_name_valid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "with_operation_name_valid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn stray_argument_invalid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "stray_argument_invalid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn unspecified_required_argument_invalid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "unspecified_required_argument_invalid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn variable_resolution_valid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "variable_resolution_valid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn variable_resolution_invalid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "variable_resolution_invalid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn invalid_subfield() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "invalid_subfield"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn aliases_valid() {
        let schema = create_test_schema().await;

        let validator = DocumentValidator::new(&schema, None, None, 10, 10);

        let query = r#"
            query {
                firstConcert: concert(id: 1) {
                    id
                    headLine: title
                }
            }
        "#;

        assert_debug!(
            validator.validate(create_query_document(query)),
            "aliases_valid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn mergeable_leaf_fields() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "mergeable_leaf_fields"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn mergeable_leaf_fields_with_alias() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "mergeable_leaf_fields_with_alias"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn unmergeable_leaf_fields_all_aliases() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "unmergeable_leaf_fields_all_aliases"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn unmergeable_leaf_fields_mixed_aliases() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "unmergeable_leaf_fields_mixed_aliases"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn mergeable_non_leaf_fields() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "mergeable_non_leaf_fields"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn unmergeable_non_leaf_fields() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "unmergeable_non_leaf_fields"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn mergeable_non_leaf_fields_with_alias() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "mergeable_non_leaf_fields_with_alias"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn multi_operations_valid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "multi_operations_valid"
        );

        let validator = DocumentValidator::new(&schema, Some("concert2".to_string()), None, 10, 10);

        assert_debug!(
            validator.validate(create_query_document(query)),
            "multi_operations_valid-2"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn multi_operations_no_operation_name_invalid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "multi_operations_no_operation_name_invalid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn multi_operations_mismatched_operation_name_invalid() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "multi_operations_mismatched_operation_name_invalid"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn fragment_recursion_direct() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "fragment_recursion_direct"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn fragment_recursion_indirect() {
        let schema = create_test_schema().await;

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

        assert_debug!(
            validator.validate(create_query_document(query)),
            "fragment_recursion_indirect"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn query_depth_limit_direct() {
        let schema = create_test_schema().await;

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
        assert_debug!(
            validator.validate(create_query_document(query)),
            "query_depth_limit_direct"
        );

        // invalid: one level too deep
        let validator = DocumentValidator::new(&schema, None, None, 5, usize::MAX);
        assert_debug!(
            validator.validate(create_query_document(query)),
            "query_depth_limit_direct-2"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn query_depth_limit_through_fragment() {
        let schema = create_test_schema().await;

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
        assert_debug!(
            validator.validate(create_query_document(query)),
            "query_depth_limit_through_fragment"
        );

        // invalid: one level too deep
        let validator = DocumentValidator::new(&schema, None, None, 5, usize::MAX);
        assert_debug!(
            validator.validate(create_query_document(query)),
            "query_depth_limit_through_fragment-2"
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn introspection_query_depth_limit_direct() {
        let schema = create_test_schema().await;

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
        assert_debug!(
            validator.validate(create_query_document(query)),
            "introspection_query_depth_limit_direct"
        );

        // invalid: one level too deep
        let validator = DocumentValidator::new(&schema, None, None, usize::MAX, 2);
        assert_debug!(
            validator.validate(create_query_document(query)),
            "introspection_query_depth_limit_direct-2"
        );
    }

    fn create_variables(variables: &str) -> Map<String, Value> {
        serde_json::from_str(variables).unwrap()
    }

    async fn create_test_schema() -> Schema {
        let test_exo = r#"
            @postgres
            module LogModule {
                @access(true)
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue
                }

                @access(true)
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    published: Boolean
                    concerts: Set<Concert>
                }
            }
        "#;
        let postgres_subsystem =
            create_postgres_system_from_str(test_exo, "test.exo".to_string()).await;

        Schema::new(
            postgres_subsystem.graphql.as_ref().unwrap().schema_types(),
            postgres_subsystem
                .graphql
                .as_ref()
                .unwrap()
                .schema_queries(),
            postgres_subsystem
                .graphql
                .as_ref()
                .unwrap()
                .schema_mutations(),
            Arc::new(None),
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
    pub async fn create_postgres_system_from_str(
        model_str: &str,
        file_name: String,
    ) -> Box<core_plugin_interface::interface::SubsystemResolver> {
        let system = builder::build_system_from_str(
            model_str,
            file_name,
            vec![Box::new(
                postgres_builder::PostgresSubsystemBuilder::default(),
            )],
            BuildMode::Build,
        )
        .await
        .unwrap();

        let subsystem_id = "postgres";

        let subsystem = system
            .subsystems
            .into_iter()
            .find(|subsystem| subsystem.id == subsystem_id)
            .unwrap();

        struct FakeConnect {}
        impl exo_sql::Connect for FakeConnect {
            fn connect(
                &self,
                _pg_config: &tokio_postgres::Config,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<
                            Output = Result<
                                (tokio_postgres::Client, tokio::task::JoinHandle<()>),
                                tokio_postgres::Error,
                            >,
                        > + Send
                        + '_,
                >,
            > {
                panic!();
            }
        }

        // Since we are not actually connecting to a database, this is fine
        // (we are only interested get queries, mutations, and types to build the schema)
        let client = DatabaseClientManager::from_connect_direct(
            false,
            tokio_postgres::Config::new(),
            FakeConnect {},
        )
        .await
        .unwrap();

        use core_plugin_interface::interface::SubsystemLoader;
        postgres_resolver::PostgresSubsystemLoader {
            existing_client: Some(client),
        }
        .init(subsystem, Arc::new(MapEnvironment::default()))
        .await
        .expect("Failed to initialize postgres subsystem")
    }
}
