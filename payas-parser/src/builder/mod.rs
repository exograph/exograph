//! Transforms an AstSystem into a GraphQL system

mod system_builder;

mod access_utils;
mod argument_builder;
mod context_builder;
mod create_mutation_builder;
mod delete_mutation_builder;
mod interceptor_weaver;
mod mutation_builder;
mod order_by_type_builder;
mod predicate_builder;
mod query_builder;
mod reference_input_type_builder;
mod resolved_builder;
mod service_builder;
mod type_builder;
mod update_mutation_builder;

pub use system_builder::build;

use payas_model::model::{mapped_arena::MappedArena, GqlType, GqlTypeKind};

use crate::builder::resolved_builder::ResolvedCompositeTypeKind;

use self::{
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
};

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
        resolved_type: &ResolvedType,
        resolved_types: &MappedArena<ResolvedType>,
        building: &mut SystemContextBuilding,
    ) {
        if let ResolvedType::Composite(c) = resolved_type {
            for type_name in self.type_names(c, resolved_types).iter() {
                building.mutation_types.add(
                    type_name,
                    GqlType {
                        name: type_name.to_string(),
                        plural_name: "".to_string(), // unused
                        kind: GqlTypeKind::Primitive,
                        is_input: true,
                    },
                );
            }
        }
    }

    fn build_shallow_only_persistent(
        &self,
        resolved_types: &MappedArena<ResolvedType>,
        building: &mut SystemContextBuilding,
    ) {
        for (_, model_type) in resolved_types.iter() {
            if let ResolvedType::Composite(ResolvedCompositeType {
                kind: ResolvedCompositeTypeKind::Persistent { .. },
                ..
            }) = &model_type
            {
                self.create_shallow_type(model_type, resolved_types, building);
            }
        }
    }

    fn build_expanded(&self, building: &mut SystemContextBuilding);
}
