use payas_model::model::{mapped_arena::MappedArena, GqlType, GqlTypeKind};

use super::{
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
};

// TODO: Ensure it works for all builders (this one makes the assumption that it is building only input types)
// TODO: Abstract out build_expanded (currently loops in it are repeated in each implementation)

/// Trait for all builders to abstract out the implementation of shallow and expanded building
pub trait Builder {
    /// Names of types produced by this builder
    fn type_names(
        &self,
        resolved_composite_type: &ResolvedCompositeType,
        models: &MappedArena<ResolvedType>,
    ) -> Vec<String>;

    fn create_shallow_type(
        &self,
        resolved_type: &ResolvedType,
        models: &MappedArena<ResolvedType>,
        building: &mut SystemContextBuilding,
    ) {
        if let ResolvedType::Composite(c) = resolved_type {
            for mutation_type_name in self.type_names(c, models).iter() {
                building.mutation_types.add(
                    mutation_type_name,
                    GqlType {
                        name: mutation_type_name.to_string(),
                        plural_name: "".to_string(), // unused
                        kind: GqlTypeKind::Primitive,
                        is_input: true,
                    },
                );
            }
        }
    }

    fn build_shallow(
        &self,
        models: &MappedArena<ResolvedType>,
        building: &mut SystemContextBuilding,
    ) {
        for (_, model_type) in models.iter() {
            self.create_shallow_type(model_type, models, building);
        }
    }

    fn build_expanded(&self, building: &mut SystemContextBuilding);
}
