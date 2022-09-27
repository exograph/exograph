use payas_core_model::mapped_arena::MappedArena;
use payas_core_model_builder::{error::ModelBuildingError, typechecker::typ::Type};
use payas_model::model::system::ModelSystem;

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
pub fn build(typechecked_system: MappedArena<Type>) -> Result<ModelSystem, ModelBuildingError> {
    let base_system =
        payas_core_model_builder::builder::system_builder::build(&typechecked_system)?;

    let database_subsystem =
        payas_database_model_builder::build(&typechecked_system, &base_system)?;

    let deno_subsystem = payas_deno_model_builder::build(&typechecked_system, &base_system)?;
    let wasm_subsystem = payas_wasm_model_builder::build(&typechecked_system, &base_system)?;

    let query_interceptors = interceptor_weaver::weave(
        database_subsystem
            .queries
            .iter()
            .map(|(_, q)| q.name.as_str())
            .chain(
                deno_subsystem
                    .underlying
                    .queries
                    .iter()
                    .map(|(_, q)| q.name.as_str()),
            ),
        &deno_subsystem.interceptors,
        &deno_subsystem.underlying.interceptors,
        OperationKind::Query,
    );

    let mutation_interceptors = interceptor_weaver::weave(
        database_subsystem
            .mutations
            .iter()
            .map(|(_, q)| q.name.as_str())
            .chain(
                deno_subsystem
                    .underlying
                    .mutations
                    .iter()
                    .map(|(_, q)| q.name.as_str()),
            ),
        &deno_subsystem.interceptors,
        &deno_subsystem.underlying.interceptors,
        OperationKind::Mutation,
    );

    Ok(ModelSystem {
        database_subsystem,
        deno_subsystem: deno_subsystem.underlying,
        wasm_subsystem: wasm_subsystem.underlying,
        query_interceptors,
        mutation_interceptors,
    })
}
