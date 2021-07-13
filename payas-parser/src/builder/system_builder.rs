use codemap::CodeMap;
use payas_model::{
    model::{
        mapped_arena::MappedArena,
        operation::{Mutation, Query},
        order::OrderByParameterType,
        predicate::PredicateParameterType,
        system::ModelSystem,
        types::GqlType,
        ContextType,
    },
    sql::PhysicalTable,
};

use crate::ast::ast_types::AstSystem;

use super::{
    context_builder, mutation_builder, order_by_type_builder, predicate_builder, query_builder,
    resolved_builder, type_builder,
};

use crate::typechecker;

pub fn build(ast_system: AstSystem, codemap: CodeMap) -> ModelSystem {
    let resolved_system = resolved_builder::build(typechecker::build(ast_system, codemap));
    let resolved_types = resolved_system.types;
    let resolved_contexts = resolved_system.contexts;

    let mut building = SystemContextBuilding::default();

    // First build shallow GQL types for model, queries, query parameters
    type_builder::build_shallow(&resolved_types, &mut building);
    context_builder::build_shallow(&resolved_contexts, &mut building);

    // The next set of shallow builders need GQL types build above (the order of the next three is unimportant)
    order_by_type_builder::build_shallow(&resolved_types, &mut building);
    predicate_builder::build_shallow(&resolved_types, &mut building);
    query_builder::build_shallow(&resolved_types, &mut building);

    // Now expand the types
    // First fully build the model types
    type_builder::build_expanded(&resolved_types, &mut building);
    context_builder::build_expanded(&resolved_contexts, &mut building);

    // Which is then used to expand query and query parameters (the order of the next three is unimportant)
    query_builder::build_expanded(&mut building);
    order_by_type_builder::build_expanded(&mut building);
    predicate_builder::build_expanded(&mut building);

    // Finally build mutations. We don't need a shallow pass, since all the types (predicates, specifically) have been already built
    mutation_builder::build(&resolved_types, &mut building);

    ModelSystem {
        types: building.types.values,
        contexts: building.contexts.values,
        order_by_types: building.order_by_types.values,
        predicate_types: building.predicate_types.values,
        queries: building.queries,
        tables: building.tables.values,
        mutation_types: building.mutation_types.values,
        create_mutations: building.mutations,
    }
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub types: MappedArena<GqlType>,
    pub contexts: MappedArena<ContextType>,
    pub order_by_types: MappedArena<OrderByParameterType>,
    pub predicate_types: MappedArena<PredicateParameterType>,
    pub queries: MappedArena<Query>,
    pub mutation_types: MappedArena<GqlType>,
    pub mutations: MappedArena<Mutation>,
    pub tables: MappedArena<PhysicalTable>,
}

#[cfg(test)]
mod tests {
    use id_arena::Arena;
    use payas_model::sql::column::{IntBits, PhysicalColumn, PhysicalColumnType};

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
        let logs_weird = get_column_from_table("weird", logs);
        let logs_prefix = get_column_from_table("prefix", logs);
        let logs_granular = get_column_from_table("granular", logs);

        // @dbtype("bigint")
        if let PhysicalColumnType::Int { bits } = &logs_id.typ {
            assert!(*bits == IntBits::_64)
        } else {
            panic!()
        }

        // @bits(16)
        if let PhysicalColumnType::Int { bits } = &logs_nonce.typ {
            assert!(*bits == IntBits::_16)
        } else {
            panic!()
        }

        // @size(8)
        if let PhysicalColumnType::Int { bits } = &logs_hash.typ {
            assert!(*bits == IntBits::_64)
        } else {
            panic!()
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
        tables: &'a Arena<PhysicalTable>,
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
        let (parsed, codemap) = parser::parse_str(src);
        build(parsed, codemap)
    }
}
