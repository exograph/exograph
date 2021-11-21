use payas_model::{
    model::{
        argument::ArgumentParameterType,
        interceptor::Interceptor,
        mapped_arena::MappedArena,
        operation::{Mutation, Query},
        order::OrderByParameterType,
        predicate::PredicateParameterType,
        service::ServiceMethod,
        system::ModelSystem,
        types::GqlType,
        ContextType,
    },
    sql::PhysicalTable,
};

use crate::ast::ast_types::{AstSystem, Untyped};

use super::{
    argument_builder, context_builder, interceptor_weaver, mutation_builder, order_by_type_builder,
    predicate_builder, query_builder,
    resolved_builder::{self, ResolvedSystem},
    service_builder, type_builder,
};

use crate::error::ParserError;
use crate::typechecker;

/// Build a [ModelSystem] given an [AstSystem].
///
/// First, it type checks the input [AstSystem] to produce typechecked types.
/// Next, it resolves the typechecked types. Resolving a type entails consuming annotations and finalizing information such as table and column names.
/// Finally, it builds the model type through a series of builders.
///
/// Each builder implements the following pattern:
/// - build_shallow: Build relevant shallow types.
///   Each shallow type in marked as primitive and thus holds just the name and notes if it is an input type.
/// - build_expanded: Fully expand the previously created shallow type as well as any other dependent objects (such as Query and Mutation)
///
/// This two pass method allows dealing with cycles.
/// In the first shallow pass, each builder iterates over resolved types and create a placeholder model type.
/// In the second expand pass, each builder again iterates over resolved types and expand each model type
/// (this is done in place, so references created from elsewhere remain valid). Since all model
/// types have been created in the first pass, the expansion pass can refer to other types (which may still be
/// shallow if hasn't had its chance in the iteration, but will expand when its turn comes in).
pub fn build(ast_system: AstSystem<Untyped>) -> Result<ModelSystem, ParserError> {
    let typechecked_system = typechecker::build(ast_system)?;
    let resolved_system = resolved_builder::build(typechecked_system)?;

    let mut building = SystemContextBuilding::default();

    build_shallow(&resolved_system, &mut building);
    build_expanded(&resolved_system, &mut building);

    Ok(ModelSystem {
        types: building.types.values,
        contexts: building.contexts.values,
        argument_types: building.argument_types.values,
        order_by_types: building.order_by_types.values,
        predicate_types: building.predicate_types.values,
        queries: building.queries,
        tables: building.tables.values,
        mutation_types: building.mutation_types.values,
        mutations: building.mutations,
        methods: building.methods.values,
    })
}

fn build_shallow(resolved_system: &ResolvedSystem, building: &mut SystemContextBuilding) {
    let resolved_types = &resolved_system.types;
    let resolved_contexts = &resolved_system.contexts;
    let resolved_services = &resolved_system.services;

    // First build shallow GQL types for types, context, query parameters (order by and predicate)
    // The order of next four is unimportant, since each of them simply create a shallow type without refering to anything
    type_builder::build_shallow(resolved_types, building);
    context_builder::build_shallow(resolved_contexts, building);
    order_by_type_builder::build_shallow(resolved_types, building);
    predicate_builder::build_shallow(resolved_types, building);
    argument_builder::build_shallow(resolved_types, building);

    // The next three shallow builders need GQL types build above (the order of the next three is unimportant)
    // Specifically, the OperationReturn type in Query, Mutation, and ServiceMethod looks for the id for the return type, so requires
    // type_builder::build_shallow to have run
    query_builder::build_shallow(resolved_types, building);
    mutation_builder::build_shallow(resolved_types, building);
    service_builder::build_shallow(resolved_types, resolved_services, building);
}

fn build_expanded(resolved_system: &ResolvedSystem, building: &mut SystemContextBuilding) {
    let resolved_types = &resolved_system.types;

    let resolved_methods = &resolved_system
        .services
        .iter()
        .map(|(_, s)| s.methods.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .concat();

    let resolved_contexts = &resolved_system.contexts;

    // First fully build the model types.
    type_builder::build_expanded(resolved_types, resolved_methods, building);
    context_builder::build_expanded(resolved_contexts, building);

    // Which is then used to expand query and query parameters (the order of the next four is unimportant) but must be executed
    // after running type_builder::build_expanded (since they depend on expanded GqlTypes (note the next ones do not access resolved_types))
    order_by_type_builder::build_expanded(building);
    predicate_builder::build_expanded(building);
    argument_builder::build_expanded(building);

    // Finally expand queries, mutations, and service methods
    query_builder::build_expanded(building);
    mutation_builder::build_expanded(building);
    service_builder::build_expanded(building);

    interceptor_weaver::weave_interceptors(resolved_system, building);
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub types: MappedArena<GqlType>,
    pub contexts: MappedArena<ContextType>,
    pub argument_types: MappedArena<ArgumentParameterType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: MappedArena<GqlType>,
    pub mutations: MappedArena<Mutation>,
    pub tables: MappedArena<PhysicalTable>,
    pub methods: MappedArena<ServiceMethod>,
    pub interceptors: MappedArena<Interceptor>,
}

#[cfg(test)]
mod tests {
    use payas_model::{
        model::mapped_arena::SerializableSlab,
        sql::column::{FloatBits, IntBits, PhysicalColumn, PhysicalColumnType},
    };

    use super::*;
    use crate::parser;

    #[test]
    fn type_hint_annotations() {
        let src = r#"
            @table("logs")
            model Log {
              id: Int @dbtype("bigint") @pk @autoincrement
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
        let parsed = parser::parse_str(src, "input.clay").unwrap();
        build(parsed).unwrap()
    }
}
