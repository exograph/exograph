use core_plugin_interface::{
    core_model::mapped_arena::{MappedArena, SerializableSlabIndex},
    core_model_builder::{
        builder::system_builder::BaseModelSystem, error::ModelBuildingError,
        typechecker::typ::TypecheckedSystem,
    },
};

use postgres_model::{
    aggregate::AggregateType,
    mutation::PostgresMutation,
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    query::{AggregateQuery, CollectionQuery, PkQuery},
    subsystem::PostgresSubsystem,
    types::{EntityType, MutationType, PostgresPrimitiveType},
};

use payas_sql::PhysicalTable;

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
            tables: building.tables.values(),
            mutation_types: building.mutation_types.values(),
            mutations: building.mutations,
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
    order_by_type_builder::build_expanded(building);
    predicate_builder::build_expanded(building);
    aggregate_type_builder::build_expanded(resolved_env, building)?;

    // Finally expand queries, mutations, and service methods
    query_builder::build_expanded(building);
    mutation_builder::build_expanded(resolved_env, building);

    Ok(())
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub primitive_types: MappedArena<PostgresPrimitiveType>,
    pub entity_types: MappedArena<EntityType>,

    pub aggregate_types: MappedArena<AggregateType>,

    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,

    pub pk_queries: MappedArena<PkQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,
    pub aggregate_queries: MappedArena<AggregateQuery>,

    pub mutation_types: MappedArena<MutationType>,
    pub mutations: MappedArena<PostgresMutation>,
    pub tables: MappedArena<PhysicalTable>,
}

impl SystemContextBuilding {
    pub fn get_entity_type_id(&self, name: &str) -> Option<SerializableSlabIndex<EntityType>> {
        self.entity_types.get_id(name)
    }
}

#[cfg(test)]
mod tests {
    use core_plugin_interface::core_model::mapped_arena::SerializableSlab;
    use payas_sql::{FloatBits, IntBits, PhysicalColumn, PhysicalColumnType};

    use super::*;

    #[test]
    fn optional_fields() {
        let src = r#"
            @postgres
            service ConcertService {
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

        let system = create_system(src);
        let get_table = |n| get_table_from_arena(n, &system.tables);

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

    #[test]
    fn one_to_one() {
        let src = r#"
        @postgres
        service UserService {
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

        let system = create_system(src);
        let get_table = |n| get_table_from_arena(n, &system.tables);

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

    #[test]
    fn type_hint_annotations() {
        let src = r#"
            @postgres
            service LogService {
                @table("logs")
                type Log {
                  @dbtype("bigint") @pk id: Int = autoIncrement() 
                  @bits(16) nonce: Int 
                  @size(8) hash: Int 
                  @size(4) float: Float 
                  @bits(40) double: Float 
                  @precision(4) latitude: Decimal 
                  @precision(5) @scale(2) longitude: Decimal 
                  @range(min=0, max=32770) weird: Int 
                  @length(15) prefix: String 
                  log: String
                  @precision(6) granular: Instant 
                }
            }
        "#;

        let system = create_system(src);
        let get_table = |n| get_table_from_arena(n, &system.tables);

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

        // Int @bits(16)
        if let PhysicalColumnType::Int { bits } = &logs_nonce.typ {
            assert!(*bits == IntBits::_16)
        } else {
            panic!()
        }

        // Int @size(8)
        if let PhysicalColumnType::Int { bits } = &logs_hash.typ {
            assert!(*bits == IntBits::_64)
        } else {
            panic!()
        }

        // Float @size(4)
        if let PhysicalColumnType::Float { bits } = &logs_float.typ {
            assert!(*bits == FloatBits::_24)
        } else {
            panic!()
        }

        // Double @bits(40)
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

        // @length(15)
        if let PhysicalColumnType::String { length } = &logs_prefix.typ {
            assert!((*length).unwrap() == 15)
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

    fn get_table_from_arena<'a>(
        name: &'a str,
        tables: &'a SerializableSlab<PhysicalTable>,
    ) -> &'a PhysicalTable {
        for (_, item) in tables.iter() {
            if item.name == name {
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

    fn create_system(src: &str) -> PostgresSubsystem {
        crate::test_utils::create_postgres_system_from_str(src, "test.exo".to_string()).unwrap()
    }
}
