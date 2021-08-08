//! Build mutation input types (<Type>CreationInput, <Type>UpdateInput, <Type>ReferenceInput) and
//! mutations (create<Type>, update<Type>, and delete<Type> as well as their plural versions)

use payas_model::model::mapped_arena::MappedArena;

use super::create_mutation_builder::CreateMutationBuilder;
use super::delete_mutation_builder::DeleteMutationBuilder;
use super::reference_input_type_builder::ReferenceInputTypeBuilder;
use super::resolved_builder::ResolvedType;
use super::system_builder::SystemContextBuilding;
use super::update_mutation_builder::UpdateMutationBuilder;

use super::builder::Builder;

// TODO: Introduce this as a struct (and have it hold the sub-builders)
// TODO: Abstract the concept of compisite builders

/// Build shallow mutaiton input types
pub fn build_shallow(models: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    ReferenceInputTypeBuilder {}.build_shallow(models, building);

    CreateMutationBuilder {}.build_shallow(models, building);
    UpdateMutationBuilder {}.build_shallow(models, building);
    DeleteMutationBuilder {}.build_shallow(models, building);
}

/// Expand the mutation input types as well as build the mutation
pub fn build_expanded(building: &mut SystemContextBuilding) {
    ReferenceInputTypeBuilder {}.build_expanded(building); // Used by many...

    CreateMutationBuilder {}.build_expanded(building);
    UpdateMutationBuilder {}.build_expanded(building);
    DeleteMutationBuilder {}.build_expanded(building);
}
