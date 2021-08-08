//! Transforms an AstSystem into a GraphQL system

mod system_builder;

mod builder;
mod context_builder;
mod create_mutation_builder;
mod delete_mutation_builder;
mod mutation_builder;
mod order_by_type_builder;
mod predicate_builder;
mod query_builder;
mod reference_input_type_builder;
mod resolved_builder;
mod type_builder;
mod update_mutation_builder;

pub use system_builder::build;
