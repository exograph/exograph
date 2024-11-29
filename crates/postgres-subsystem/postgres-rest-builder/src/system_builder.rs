// use std::cell::RefCell;

// use core_plugin_interface::{
//     core_model::{
//         access::AccessPredicateExpression,
//         mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex},
//     },
//     core_model_builder::{
//         builder::{resolved_builder, system_builder::BaseModelSystem},
//         error::ModelBuildingError,
//         plugin::RestSubsystemBuild,
//         typechecker::typ::TypecheckedSystem,
//     },
// };

// pub fn build(
//     typechecked_system: &TypecheckedSystem,
//     base_system: &BaseModelSystem,
// ) -> Result<Option<RestSubsystemBuild>, ModelBuildingError> {
//     let mut building = SystemContextBuilding::default();

//     let resolved_types = resolved_builder::build(typechecked_system)?;
// }
