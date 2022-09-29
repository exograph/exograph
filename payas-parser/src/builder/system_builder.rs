use payas_core_model::{
    mapped_arena::MappedArena,
    system::{Subsystem, System},
    system_serializer::SystemSerializer,
};
use payas_core_model_builder::{
    error::ModelBuildingError, plugin::SubsystemBuilder, typechecker::typ::Type,
};

use super::interceptor_weaver::{self, OperationKind};

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
pub fn build(typechecked_system: MappedArena<Type>) -> Result<Vec<u8>, ModelBuildingError> {
    let base_system =
        payas_core_model_builder::builder::system_builder::build(&typechecked_system)?;

    let database_subsystem_builder = payas_database_model_builder::DatabaseSubsystemBuilder {};
    let deno_subsystem_builder = payas_deno_model_builder::DenoSubsystemBuilder {};
    let wasm_subsystem_builder = payas_wasm_model_builder::WasmSubsystemBuilder {};

    let subsystem_builders: Vec<&dyn SubsystemBuilder> = vec![
        &database_subsystem_builder,
        &deno_subsystem_builder,
        &wasm_subsystem_builder,
    ];

    let mut subsystem_interceptions = vec![];
    let mut query_names = vec![];
    let mut mutation_names = vec![];

    let subsystems: Vec<Subsystem> = subsystem_builders
        .iter()
        .enumerate()
        .map(|(subsystem_index, builder)| {
            let build_info = builder.build(&typechecked_system, &base_system)?;

            subsystem_interceptions.push((subsystem_index, build_info.interceptions));
            query_names.extend(build_info.query_names);
            mutation_names.extend(build_info.mutation_names);

            Ok(Subsystem {
                id: build_info.id,
                subsystem_index,
                serialized_subsystem: build_info.serialized_subsystem,
            })
        })
        .collect::<Result<Vec<_>, ModelBuildingError>>()?;

    let query_interception_map = interceptor_weaver::weave(
        query_names.iter().map(|n| n.as_str()),
        &subsystem_interceptions,
        OperationKind::Query,
    );

    let mutation_interception_map = interceptor_weaver::weave(
        mutation_names.iter().map(|n| n.as_str()),
        &subsystem_interceptions,
        OperationKind::Mutation,
    );

    let system = System {
        subsystems,
        query_interception_map,
        mutation_interception_map,
    };

    system.serialize().map_err(ModelBuildingError::Serialize)
}
