use payas_core_model_builder::{error::ModelBuildingError, typechecker::typ::Type};
use payas_model::model::{mapped_arena::MappedArena, system::ModelSystem};

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

    let mut database_system =
        payas_database_model_builder::build(&typechecked_system, &base_system)?;

    let mut service_system = payas_service_model_builder::build(&typechecked_system, &base_system)?;

    interceptor_weaver::weave_queries(&mut database_system.queries, &service_system.interceptors);
    interceptor_weaver::weave_mutations(
        &mut database_system.mutations,
        &service_system.interceptors,
    );
    interceptor_weaver::weave_queries(&mut service_system.queries, &service_system.interceptors);
    interceptor_weaver::weave_mutations(
        &mut service_system.mutations,
        &service_system.interceptors,
    );

    Ok(ModelSystem {
        primitive_types: base_system.primitive_types.values,
        database_types: database_system.database_types,
        service_types: service_system.service_types,

        contexts: base_system.contexts,
        context_types: base_system.context_types.values,
        argument_types: service_system.argument_types,
        order_by_types: database_system.order_by_types,
        predicate_types: database_system.predicate_types,
        database_queries: database_system.queries,
        database_mutations: database_system.mutations,
        service_queries: service_system.queries,
        service_mutations: service_system.mutations,
        tables: database_system.tables,
        mutation_types: database_system.mutation_types,
        methods: service_system.methods,
        scripts: service_system.scripts,
    })
}
