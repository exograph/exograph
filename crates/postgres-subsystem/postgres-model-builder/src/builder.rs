//! Transforms an AstSystem into a GraphQL system

use core_plugin_interface::core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use postgres_model::{access::Access, types::PostgresCompositeType};

use crate::shallow::Shallow;

use super::{
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
    type_builder::ResolvedTypeEnv,
};

pub const DEFAULT_FN_AUTOINCREMENT: &str = "autoincrement";
pub const DEFAULT_FN_CURRENT_TIME: &str = "now";
pub const DEFAULT_FN_GENERATE_UUID: &str = "generate_uuid";

// TODO: Ensure it works for all builders (this one makes the assumption that it is building only input types)
// TODO: Abstract out build_expanded (currently loops in it are repeated in each implementation)

/// Trait for all builders to abstract out the implementation of shallow and expanded building
pub trait Builder {
    /// Names of types produced by this builder.
    /// Shallow building use these type names (since not much else is needed)
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        resolved_types: &MappedArena<ResolvedType>,
    ) -> Vec<String>;

    fn create_shallow_type(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        resolved_types: &MappedArena<ResolvedType>,
        building: &mut SystemContextBuilding,
    ) {
        for type_name in self
            .type_names(resolved_composite_type, resolved_types)
            .iter()
        {
            building.mutation_types.add(
                type_name,
                PostgresCompositeType {
                    name: type_name.to_string(),
                    fields: vec![],
                    agg_fields: vec![],
                    pk_query: SerializableSlabIndex::shallow(),
                    collection_query: SerializableSlabIndex::shallow(),
                    aggregate_query: SerializableSlabIndex::shallow(),
                    table_id: SerializableSlabIndex::shallow(),
                    plural_name: "".to_string(), // unused
                    is_input: true,
                    access: Access::restrictive(),
                },
            );
        }
    }

    fn build_shallow(
        &self,
        resolved_types: &MappedArena<ResolvedType>,
        building: &mut SystemContextBuilding,
    ) {
        for (_, model_type) in resolved_types.iter() {
            if let ResolvedType::Composite(composite_type) = &model_type {
                self.create_shallow_type(composite_type, resolved_types, building);
            }
        }
    }

    fn build_expanded(&self, resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding);
}
