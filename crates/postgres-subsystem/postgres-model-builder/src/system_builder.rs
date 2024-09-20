// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::cell::RefCell;

use core_plugin_interface::{
    core_model::{
        access::AccessPredicateExpression,
        mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex},
    },
    core_model_builder::{
        builder::system_builder::BaseModelSystem, error::ModelBuildingError,
        typechecker::typ::TypecheckedSystem,
    },
};

use postgres_model::{
    access::{DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression},
    aggregate::AggregateType,
    mutation::PostgresMutation,
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    query::{AggregateQuery, CollectionQuery, PkQuery, UniqueQuery},
    subsystem::PostgresSubsystem,
    types::{EntityType, MutationType, PostgresPrimitiveType},
    vector_distance::VectorDistanceType,
};

use exo_sql::Database;

use crate::aggregate_type_builder;

use super::{
    mutation_builder, order_by_type_builder, predicate_builder, query_builder, resolved_builder,
    type_builder, type_builder::ResolvedTypeEnv,
};

pub fn build(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
) -> Result<Option<PostgresSubsystem>, ModelBuildingError> {
    let mut building = SystemContextBuilding::default();

    let resolved_types = resolved_builder::build(typechecked_system)?;
    let system = {
        let resolved_env = ResolvedTypeEnv {
            contexts: &base_system.contexts,
            resolved_types,
            function_definitions: &base_system.function_definitions,
        };

        build_shallow(&resolved_env, &mut building);
        build_expanded(&resolved_env, &mut building)?;

        PostgresSubsystem {
            contexts: base_system.contexts.clone(),
            primitive_types: building.primitive_types.values(),
            entity_types: building.entity_types.values(),
            aggregate_types: building.aggregate_types.values(),

            order_by_types: building.order_by_types.values(),
            predicate_types: building.predicate_types.values(),
            pk_queries: building.pk_queries,
            collection_queries: building.collection_queries,
            aggregate_queries: building.aggregate_queries,
            unique_queries: building.unique_queries,
            database: building.database,
            mutation_types: building.mutation_types.values(),
            mutations: building.mutations,

            input_access_expressions: building.input_access_expressions.into_inner().elems,
            database_access_expressions: building.database_access_expressions.into_inner().elems,
        }
    };

    Ok({
        if system.pk_queries.is_empty()
            && system.collection_queries.is_empty()
            && system.aggregate_queries.is_empty()
            && system.mutations.is_empty()
        {
            None
        } else {
            Some(system)
        }
    })
}

/// Build shallow types, context, query parameters (order by and predicate)
fn build_shallow(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    // The order of next three is unimportant, since each of them simply create a shallow type without referring to anything
    type_builder::build_shallow(resolved_env, building);

    order_by_type_builder::build_shallow(resolved_env, building);

    predicate_builder::build_shallow(&resolved_env.resolved_types, building);

    aggregate_type_builder::build_shallow(resolved_env, building);

    // The next two shallow builders need POSTGRES types build above (the order of the next two is unimportant)
    // Specifically, the OperationReturn type in Query and Mutation looks for the id for the return type, so requires
    // type_builder::build_shallow to have run
    query_builder::build_shallow(&resolved_env.resolved_types, building);
    mutation_builder::build_shallow(&resolved_env.resolved_types, building);
}

fn build_expanded(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    // First fully build the types.
    type_builder::build_expanded(resolved_env, building)?;

    // Which is then used to expand query and query parameters (the order is unimportant) but must be executed
    // after running type_builder::build_expanded (since they depend on expanded PostgresTypes (note the next ones do not access resolved_types))
    order_by_type_builder::build_expanded(resolved_env, building);
    predicate_builder::build_expanded(resolved_env, building);
    aggregate_type_builder::build_expanded(resolved_env, building)?;

    // Finally expand queries, mutations, and module methods
    query_builder::build_expanded(resolved_env, building);
    mutation_builder::build_expanded(building)?;

    Ok(())
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub primitive_types: MappedArena<PostgresPrimitiveType>,
    pub entity_types: MappedArena<EntityType>,

    pub aggregate_types: MappedArena<AggregateType>,
    pub vector_distance_types: MappedArena<VectorDistanceType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,

    pub pk_queries: MappedArena<PkQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,
    pub aggregate_queries: MappedArena<AggregateQuery>,
    pub unique_queries: MappedArena<UniqueQuery>,

    pub mutation_types: MappedArena<MutationType>,
    pub mutations: MappedArena<PostgresMutation>,

    pub input_access_expressions:
        RefCell<AccessExpressionsBuilding<InputAccessPrimitiveExpression>>,
    pub database_access_expressions:
        RefCell<AccessExpressionsBuilding<DatabaseAccessPrimitiveExpression>>,

    pub database: Database,
}

/// Structure to keep track of access expressions arena and a special index for the oft-used restrictive access.
/// By keeping track of the restrictive access index, we avoid creating multiple indices for the same `False` expression.
#[derive(Debug)]
pub struct AccessExpressionsBuilding<T: Send + Sync> {
    elems: SerializableSlab<AccessPredicateExpression<T>>,
    restrictive_access_index: SerializableSlabIndex<AccessPredicateExpression<T>>,
}

impl<T: Send + Sync> AccessExpressionsBuilding<T> {
    pub fn insert(
        &mut self,
        elem: AccessPredicateExpression<T>,
    ) -> SerializableSlabIndex<AccessPredicateExpression<T>> {
        self.elems.insert(elem)
    }

    pub fn restricted_access_index(&self) -> SerializableSlabIndex<AccessPredicateExpression<T>> {
        self.restrictive_access_index
    }
}

impl<T: Send + Sync> Default for AccessExpressionsBuilding<T> {
    fn default() -> Self {
        let mut elems = SerializableSlab::new();
        // Insert a default restrictive access expression and keep around its index
        let restrictive_access_index =
            elems.insert(AccessPredicateExpression::BooleanLiteral(false));
        Self {
            elems,
            restrictive_access_index,
        }
    }
}

impl<T: Send + Sync> core::ops::Index<SerializableSlabIndex<AccessPredicateExpression<T>>>
    for AccessExpressionsBuilding<T>
{
    type Output = AccessPredicateExpression<T>;

    fn index(&self, index: SerializableSlabIndex<AccessPredicateExpression<T>>) -> &Self::Output {
        &self.elems[index]
    }
}

impl SystemContextBuilding {
    pub fn get_entity_type_id(&self, name: &str) -> Option<SerializableSlabIndex<EntityType>> {
        self.entity_types.get_id(name)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use exo_sql::{FloatBits, IntBits, PhysicalColumn, PhysicalColumnType, PhysicalTable};

    use super::*;

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
        let get_table = |n| get_table_from_arena(n, &system.database);

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
        let get_table = |n| get_table_from_arena(n, &system.database);

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

    fn get_mutation_type_names(system: &PostgresSubsystem) -> HashSet<String> {
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
        let get_table = |n| get_table_from_arena(n, &system.database);

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

    async fn create_system(src: &str) -> PostgresSubsystem {
        crate::test_utils::create_postgres_system_from_str(src, "test.exo".to_string())
            .await
            .unwrap()
    }
}
