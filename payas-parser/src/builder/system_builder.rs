use payas_core_model::mapped_arena::MappedArena;
use payas_core_model_builder::{error::ModelBuildingError, typechecker::typ::Type};
use payas_model::model::system::ModelSystem;

use super::interceptor_weaver;

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
pub fn build(typechecked_system: MappedArena<Type>) -> Result<ModelSystem, ModelBuildingError> {
    let base_system =
        payas_core_model_builder::builder::system_builder::build(&typechecked_system)?;

    let database_subsystem =
        payas_database_model_builder::build(&typechecked_system, &base_system)?;

    let service_subsystem = payas_service_model_builder::build(&typechecked_system, &base_system)?;

    // interceptor_weaver::weave_queries(&mut database_system.queries, &service_system.interceptors);
    // interceptor_weaver::weave_mutations(
    //     &mut database_system.mutations,
    //     &service_system.interceptors,
    // );
    // interceptor_weaver::weave_queries(&mut service_system.queries, &service_system.interceptors);
    // interceptor_weaver::weave_mutations(
    //     &mut service_system.mutations,
    //     &service_system.interceptors,
    // );

    Ok(ModelSystem {
        database_subsystem,
        service_subsystem: service_subsystem.underlying,
    })
}
