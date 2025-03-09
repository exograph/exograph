use std::sync::Arc;

use async_trait::async_trait;
use core_plugin_interface::{
    core_model_builder::{
        builder::system_builder::BaseModelSystem,
        error::ModelBuildingError,
        plugin::CoreSubsystemBuild,
        typechecker::{
            annotation::{AnnotationSpec, AnnotationTarget, MappedAnnotationParamSpec},
            typ::TypecheckedSystem,
        },
    },
    interface::{SubsystemBuild, SubsystemBuilder},
    serializable_system::SerializableCoreBytes,
    system_serializer::SystemSerializer,
};
use postgres_core_builder::resolved_type::ResolvedTypeEnv;
use postgres_graphql_builder::PostgresGraphQLSubsystemBuilder;
use postgres_rest_builder::PostgresRestSubsystemBuilder;

pub struct PostgresSubsystemBuilder {
    pub graphql_builder: Option<PostgresGraphQLSubsystemBuilder>,
    pub rest_builder: Option<PostgresRestSubsystemBuilder>,
}

impl Default for PostgresSubsystemBuilder {
    fn default() -> Self {
        Self {
            graphql_builder: Some(PostgresGraphQLSubsystemBuilder {}),
            rest_builder: Some(PostgresRestSubsystemBuilder {}),
        }
    }
}

#[async_trait]
impl SubsystemBuilder for PostgresSubsystemBuilder {
    fn id(&self) -> &'static str {
        "postgres"
    }

    fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![
            (
                "postgres",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Module],
                    no_params: true,
                    single_params: false,
                    mapped_params: Some(&[
                        MappedAnnotationParamSpec {
                            name: "schema",
                            optional: true,
                        },
                        MappedAnnotationParamSpec {
                            name: "managed",
                            optional: true,
                        },
                    ]),
                },
            ),
            (
                "column",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "relation",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "dbtype",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "maxLength",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "pk",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "manyToOne",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "oneToOne",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "plural",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Type],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "precision",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "range",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: false,
                    mapped_params: Some(&[
                        MappedAnnotationParamSpec {
                            name: "min",
                            optional: false,
                        },
                        MappedAnnotationParamSpec {
                            name: "max",
                            optional: false,
                        },
                    ]),
                },
            ),
            (
                "scale",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "bits16",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "bits32",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "bits64",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "singlePrecision",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "size", // vector size
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "distanceFunction", // vector distance function
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "doublePrecision",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "table",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Type],
                    no_params: false,
                    single_params: true,
                    mapped_params: Some(&[
                        MappedAnnotationParamSpec {
                            name: "name",
                            optional: true,
                        },
                        MappedAnnotationParamSpec {
                            name: "schema",
                            optional: true,
                        },
                        MappedAnnotationParamSpec {
                            name: "managed",
                            optional: true,
                        },
                    ]),
                },
            ),
            (
                "unique",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "index",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "readonly",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "update",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "json",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Type],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
        ]
    }

    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        base_system: &BaseModelSystem,
    ) -> Result<Option<SubsystemBuild>, ModelBuildingError> {
        let resolved_types = postgres_core_builder::resolved_builder::build(typechecked_system)?;

        let resolved_env = ResolvedTypeEnv {
            contexts: &base_system.contexts,
            resolved_types,
            function_definitions: &base_system.function_definitions,
        };

        let core_subsystem_building =
            Arc::new(postgres_core_builder::system_builder::build(&resolved_env)?);

        let graphql_subsystem = match self.graphql_builder.as_ref() {
            Some(builder) => {
                builder
                    .build(&resolved_env, core_subsystem_building.clone())
                    .await
            }
            None => Ok(None),
        }?;

        let rest_subsystem = match self.rest_builder.as_ref() {
            Some(builder) => {
                builder
                    .build(&resolved_env, core_subsystem_building.clone())
                    .await
            }
            None => Ok(None),
        }?;

        let serialized_core_subsystem = {
            let core_subsystem = Arc::into_inner(core_subsystem_building)
                .unwrap()
                .into_core_subsystem(base_system);
            core_subsystem
                .serialize()
                .map_err(ModelBuildingError::Serialize)?
        };

        if graphql_subsystem.is_none() && rest_subsystem.is_none() {
            Ok(None)
        } else {
            Ok(Some(SubsystemBuild {
                id: "postgres",
                graphql: graphql_subsystem,
                rest: rest_subsystem,
                core: CoreSubsystemBuild {
                    serialized_subsystem: SerializableCoreBytes(serialized_core_subsystem),
                },
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use exo_sql::{
        Database, FloatBits, IntBits, PhysicalColumn, PhysicalColumnType, PhysicalTable,
    };
    use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn optional_fields() {
        let src = r#"
            @postgres
            module ConcertModule {
                @table("concerts")
                type Concert {
                    @pk id: Int = autoIncrement()
                    title: String
                    venue: Venue?
                    icon: Blob?
                }

                @table("venues")
                type Venue {
                    @pk id: Int = autoIncrement()
                    name: String
                    address: String?
                    concerts: Set<Concert>?
                }
            }
        "#;

        let system = create_system(src).await;
        let get_table = |n| get_table_from_arena(n, &system.core_subsystem.database);

        let concerts = get_table("concerts");
        let venues = get_table("venues");

        // pks should just have PRIMARY KEY constraint, not NOT NULL
        let concerts_id = get_column_from_table("id", concerts);
        let venues_id = get_column_from_table("id", venues);
        assert!(concerts_id.is_pk);
        assert!(venues_id.is_pk);

        // NOT NULL default
        let concerts_title = get_column_from_table("title", concerts);
        let venues_name = get_column_from_table("name", venues);
        assert!(!concerts_title.is_nullable);
        assert!(!venues_name.is_nullable);

        // NOT NULL when field is marked with '?'
        let concerts_venue = get_column_from_table("venue_id", concerts); // composite type field (ManyToOne)
        let concerts_icon = get_column_from_table("icon", concerts); // primitive type field

        // OneToMany fields don't exist in database
        let venues_address = get_column_from_table("address", venues); // primitive type field

        assert!(concerts_venue.is_nullable);
        assert!(concerts_icon.is_nullable);

        assert!(venues_address.is_nullable);
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn one_to_one() {
        let src = r#"
        @postgres
        module UserModule {
            type User {
                @pk id: Int = autoIncrement()
                membership: Membership?
            }

            type Membership {
                @pk id: Int = autoIncrement()
                user: User
            }
        }
        "#;

        let system = create_system(src).await;
        println!(
            "Database tables {:?}",
            system.core_subsystem.database.tables().len()
        );
        let get_table = |n| get_table_from_arena(n, &system.core_subsystem.database);

        let users = get_table("users");
        let memberships = get_table("memberships");

        // pks should just have PRIMARY KEY constraint, not NOT NULL
        let users_id = get_column_from_table("id", users);
        let memberships_id = get_column_from_table("id", memberships);
        assert!(users_id.is_pk);
        assert!(memberships_id.is_pk);

        let users_membership = get_column_from_table("user_id", memberships);
        assert!(!users_membership.is_nullable);
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn access_false() {
        let src = r#"
        @postgres
        module TodoModule {
            @access(false)
            type Todo {
                @pk id: Int = autoIncrement()
                title: String
            }
        }
        "#;

        let system = create_system(src).await;
        assert!(system.mutations.is_empty());
        let mutation_type_names = get_mutation_type_names(&system);
        assert!(!mutation_type_names.contains("TodoUpdateInput"));
        assert!(!mutation_type_names.contains("TodoCreationInput"));
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn access_false_mutation() {
        let src = r#"
        @postgres
        module TodoModule {
            @access(query=true, mutation=false)
            type Todo {
                @pk id: Int = autoIncrement()
                title: String
            }
        }
        "#;

        let system = create_system(src).await;
        assert!(system.mutations.is_empty());
        let mutation_type_names = get_mutation_type_names(&system);
        assert!(!mutation_type_names.contains("TodoUpdateInput"));
        assert!(!mutation_type_names.contains("TodoCreationInput"));
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn access_false_create_mutation() {
        let src = r#"
        @postgres
        module TodoModule {
            @access(query=true, mutation=true, create=false)
            type Todo {
                @pk id: Int = autoIncrement()
                title: String
            }
        }
        "#;

        let system = create_system(src).await;
        assert!(system.mutations.get_by_key("createTodo").is_none());
        assert!(system.mutations.get_by_key("createTodos").is_none());
        let mutation_type_names = get_mutation_type_names(&system);
        assert!(!mutation_type_names.contains("TodoCreationInput"));
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn access_false_delete_mutation() {
        let src = r#"
        @postgres
        module TodoModule {
            @access(query=true, mutation=true, delete=false)
            type Todo {
                @pk id: Int = autoIncrement()
                title: String
            }
        }
        "#;

        let system = create_system(src).await;
        assert!(system.mutations.get_by_key("deleteTodo").is_none());
        assert!(system.mutations.get_by_key("deleteTodos").is_none());
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn access_false_update_mutation() {
        let src = r#"
        @postgres
        module TodoModule {
            @access(query=true, mutation=true, update=false)
            type Todo {
                @pk id: Int = autoIncrement()
                title: String
            }
        }
        "#;

        let system = create_system(src).await;
        assert!(system.mutations.get_by_key("updateTodo").is_none());
        assert!(system.mutations.get_by_key("updateTodos").is_none());
        let mutation_type_names = get_mutation_type_names(&system);
        assert!(!mutation_type_names.contains("TodoUpdateInput"));
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn self_referencing_through_matching_field() {
        let src = r#"
        @postgres
        module CompanyModule {
            @access(true)
            type Employee {
                @pk id: Int = autoIncrement()
                name: String
                manager: Employee?
                reports: Set<Employee>?
            }
        }
        "#;

        let system = create_system(src).await;
        let mutation_type_names = get_mutation_type_names(&system);

        assert!(mutation_type_names.contains("EmployeeReferenceInput"));
        assert!(mutation_type_names.contains("EmployeeUpdateInput"));
        assert!(mutation_type_names.contains("EmployeeCreationInput"));
        assert!(mutation_type_names.contains("EmployeeCreationInputFromEmployee"));
        assert!(mutation_type_names.contains("EmployeeUpdateInputFromEmployee"));
        assert!(mutation_type_names.contains("EmployeeUpdateInputFromEmployeeNested"));
    }

    fn get_mutation_type_names(system: &PostgresGraphQLSubsystem) -> HashSet<String> {
        system
            .mutation_types
            .iter()
            .map(|(_, t)| t.name.clone())
            .collect::<HashSet<String>>()
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn type_hint_annotations() {
        let src = r#"
            @postgres
            module LogModule {
                @table("logs")
                type Log {
                  @dbtype("bigint") @pk id: Int = autoIncrement()
                  @bits16 nonce: Int
                  @bits64 hash: Int
                  @singlePrecision float: Float
                  @doublePrecision double: Float
                  @precision(4) latitude: Decimal
                  @precision(5) @scale(2) longitude: Decimal
                  @range(min=0, max=32770) weird: Int
                  @maxLength(15) prefix: String
                  log: String
                  @precision(6) granular: Instant
                }
            }
        "#;

        let system = create_system(src).await;
        let get_table = |n| get_table_from_arena(n, &system.core_subsystem.database);

        let logs = get_table("logs");
        let logs_id = get_column_from_table("id", logs);
        let logs_nonce = get_column_from_table("nonce", logs);
        let logs_hash = get_column_from_table("hash", logs);
        let logs_float = get_column_from_table("float", logs);
        let logs_double = get_column_from_table("double", logs);
        let logs_latitude = get_column_from_table("latitude", logs);
        let logs_longitude = get_column_from_table("longitude", logs);
        let logs_weird = get_column_from_table("weird", logs);
        let logs_prefix = get_column_from_table("prefix", logs);
        let logs_granular = get_column_from_table("granular", logs);

        // @dbtype("bigint")
        if let PhysicalColumnType::Int { bits } = &logs_id.typ {
            assert!(*bits == IntBits::_64)
        } else {
            panic!()
        }

        // Int @bits16
        if let PhysicalColumnType::Int { bits } = &logs_nonce.typ {
            assert!(*bits == IntBits::_16)
        } else {
            panic!()
        }

        // Int @bits64
        if let PhysicalColumnType::Int { bits } = &logs_hash.typ {
            assert!(*bits == IntBits::_64)
        } else {
            panic!()
        }

        // Float @singlePrecision
        if let PhysicalColumnType::Float { bits } = &logs_float.typ {
            assert!(*bits == FloatBits::_24)
        } else {
            panic!()
        }

        // Double @doublePrecision
        if let PhysicalColumnType::Float { bits } = &logs_double.typ {
            assert!(*bits == FloatBits::_53)
        } else {
            panic!()
        }

        // Decimal @precision(4)
        if let PhysicalColumnType::Numeric { precision, scale } = &logs_latitude.typ {
            assert!(*precision == Some(4));
            assert!(scale.is_none());
        }

        // Decimal @precision(5) @scale(2)
        if let PhysicalColumnType::Numeric { precision, scale } = &logs_longitude.typ {
            assert!(*precision == Some(5));
            assert!(*scale == Some(2));
        }

        // @range(min=0, max=32770)
        if let PhysicalColumnType::Int { bits } = &logs_weird.typ {
            // range in hint does NOT fit in SMALLINT
            assert!(*bits == IntBits::_32)
        } else {
            panic!()
        }

        // @maxLength(15)
        if let PhysicalColumnType::String { max_length } = &logs_prefix.typ {
            assert!((*max_length).unwrap() == 15)
        } else {
            panic!()
        }

        // @precision(6)
        match &logs_granular.typ {
            PhysicalColumnType::Timestamp { precision, .. } => {
                if let Some(p) = precision {
                    assert!(*p == 6)
                } else {
                    panic!()
                }
            }
            _ => panic!(),
        };
    }

    fn get_table_from_arena<'a>(name: &'a str, database: &'a Database) -> &'a PhysicalTable {
        for (_, item) in database.tables().iter() {
            if item.name.name == name {
                return item;
            }
        }

        panic!("No such table {name}")
    }

    fn get_column_from_table<'a>(name: &'a str, table: &'a PhysicalTable) -> &'a PhysicalColumn {
        for item in table.columns.iter() {
            if item.name == name {
                return item;
            }
        }

        panic!("No such column {name}")
    }

    async fn create_system(src: &str) -> PostgresGraphQLSubsystem {
        crate::test_utils::create_postgres_system_from_str(src, "test.exo".to_string())
            .await
            .unwrap()
    }
}
