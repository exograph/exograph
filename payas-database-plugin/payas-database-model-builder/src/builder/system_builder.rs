use payas_model::model::{
    mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex},
    operation::{Mutation, Query},
    order::OrderByParameterType,
    predicate::PredicateParameterType,
    types::GqlType,
};

use payas_sql::PhysicalTable;

use super::{
    mutation_builder, order_by_type_builder, predicate_builder, query_builder,
    resolved_builder::{self, ResolvedDatabaseSystem},
    type_builder::{self},
};

use payas_core_model_builder::{
    builder::{
        resolved_builder::ResolvedType, system_builder::BaseModelSystem,
        type_builder::ResolvedTypeEnv,
    },
    error::ModelBuildingError,
    typechecker::typ::{PrimitiveType, Type},
};

pub struct ModelDatabaseSystem {
    pub database_types: SerializableSlab<GqlType>,

    // query related
    pub order_by_types: SerializableSlab<OrderByParameterType>,
    pub predicate_types: SerializableSlab<PredicateParameterType>,
    pub queries: MappedArena<Query>,

    // mutation related
    pub mutation_types: SerializableSlab<GqlType>, // create, update, delete input types such as `PersonUpdateInput`
    pub mutations: MappedArena<Mutation>,

    pub tables: SerializableSlab<PhysicalTable>,
}

pub fn build(
    typechecked_system: &MappedArena<Type>,
    base_system: &BaseModelSystem,
) -> Result<ModelDatabaseSystem, ModelBuildingError> {
    let mut building = SystemContextBuilding::default();

    let resolved_system = resolved_builder::build(&typechecked_system)?;

    let mut resolved_primitive_types = MappedArena::default();

    base_system.primitive_types.iter().for_each(|(_, typ)| {
        resolved_primitive_types.add(
            typ.name.as_str(),
            ResolvedType::Primitive(PrimitiveType::from_str(typ.name.as_str())),
        );
    });

    let resolved_env = ResolvedTypeEnv {
        base_system,
        resolved_subsystem_types: &resolved_system.database_types,
    };

    build_shallow_database(&resolved_system, &resolved_env, &mut building);
    build_expanded_database(&resolved_env, &mut building)?;

    Ok(ModelDatabaseSystem {
        database_types: building.database_types.values,

        order_by_types: building.order_by_types.values,
        predicate_types: building.predicate_types.values,
        queries: building.queries,
        tables: building.tables.values,
        mutation_types: building.mutation_types.values,
        mutations: building.mutations,
    })
}

fn build_shallow_database(
    resolved_system: &ResolvedDatabaseSystem,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    let resolved_database_types = &resolved_system.database_types;

    // First build shallow GQL types for types, context, query parameters (order by and predicate)
    // The order of next five is unimportant, since each of them simply create a shallow type without referring to anything
    type_builder::build_shallow(resolved_database_types, resolved_env, building);

    order_by_type_builder::build_shallow(resolved_database_types, building);

    building.predicate_types = predicate_builder::build_primitive_predicates(
        resolved_env
            .base_system
            .primitive_types
            .values
            .iter()
            .map(|t| t.1),
    );
    predicate_builder::build_shallow(resolved_database_types, resolved_env, building);

    // The next two shallow builders need GQL types build above (the order of the next three is unimportant)
    // Specifically, the OperationReturn type in Query and Mutation looks for the id for the return type, so requires
    // type_builder::build_shallow to have run
    query_builder::build_shallow(resolved_database_types, resolved_env, building);
    mutation_builder::build_shallow(resolved_database_types, building);
}

fn build_expanded_database(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    // First fully build the model types.
    type_builder::build_persistent_expanded(resolved_env, building)?;

    // Which is then used to expand query and query parameters (the order of the next four is unimportant) but must be executed
    // after running type_builder::build_expanded (since they depend on expanded GqlTypes (note the next ones do not access resolved_types))
    order_by_type_builder::build_expanded(resolved_env, building);
    predicate_builder::build_expanded(resolved_env, building);

    // Finally expand queries, mutations, and service methods
    query_builder::build_expanded(resolved_env, building);
    mutation_builder::build_expanded(resolved_env, building);

    Ok(())
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub database_types: MappedArena<GqlType>,

    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,

    // break this into subsystems
    pub queries: MappedArena<Query>,

    pub mutation_types: MappedArena<GqlType>,
    pub mutations: MappedArena<Mutation>,
    pub tables: MappedArena<PhysicalTable>,
}

impl SystemContextBuilding {
    pub fn get_id(
        &self,
        name: &str,
        resolved_env: &ResolvedTypeEnv,
    ) -> Option<SerializableSlabIndex<GqlType>> {
        resolved_env
            .base_system
            .primitive_types
            .get_id(name)
            .or_else(|| self.database_types.get_id(name))
            .or_else(|| resolved_env.base_system.context_types.get_id(name))
    }
}

#[cfg(test)]
mod tests {
    use payas_model::model::mapped_arena::SerializableSlab;
    use payas_sql::{FloatBits, IntBits, PhysicalColumn, PhysicalColumnType};

    use super::*;

    #[test]
    fn optional_fields() {
        let src = r#"
            @table("concerts")
            model Concert {
                id: Int = autoincrement() @pk
                title: String
                venue: Venue?
                icon: Blob?
            }

            @table("venues")
            model Venue {
                id: Int = autoincrement() @pk
                name: String
                address: String?
                concerts: Set<Concert>?
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
            model User {
                id: Int = autoincrement() @pk
                membership: Membership?
            }

            model Membership {
                id: Int = autoincrement() @pk
                user: User
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
            @table("logs")
            model Log {
              id: Int = autoincrement() @dbtype("bigint") @pk
              nonce: Int @bits(16)
              hash: Int @size(8)
              float: Float @size(4)
              double: Float @bits(40)
              latitude: Decimal @precision(4)
              longitude: Decimal @precision(5) @scale(2)
              weird: Int @range(min=0, max=32770)
              prefix: String @length(15)
              log: String
              granular: Instant @precision(6)
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
            assert!(*scale == None);
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

        panic!("No such table {}", name)
    }

    fn get_column_from_table<'a>(name: &'a str, table: &'a PhysicalTable) -> &'a PhysicalColumn {
        for item in table.columns.iter() {
            if item.column_name == name {
                return item;
            }
        }

        panic!("No such column {}", name)
    }

    fn create_system(src: &str) -> ModelSystem {
        crate::build_system_from_str(src, "input.clay".to_string()).unwrap()
    }
}
