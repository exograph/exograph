#![cfg(test)]

use codemap::CodeMap;
use core_model_builder::{
    builder::system_builder::BaseModelSystem, error::ModelBuildingError,
    typechecker::typ::TypecheckedSystem,
};
use core_plugin_interface::core_model::mapped_arena::MappedArena;
use resolved_type::{ResolvedType, ResolvedTypeEnv};

use super::*;
use builder::{load_subsystem_builders, parser, typechecker};

pub(crate) fn create_typechecked_system_from_src(
    src: &str,
) -> Result<TypecheckedSystem, ModelBuildingError> {
    let mut codemap = CodeMap::new();
    let subsystem_builders = load_subsystem_builders(vec![
        Box::new(postgres_builder::PostgresSubsystemBuilder::default()),
        #[cfg(not(target_family = "wasm"))]
        Box::new(deno_builder::DenoSubsystemBuilder::default()),
    ])
    .unwrap();
    let parsed = parser::parse_str(src, &mut codemap, "input.exo")
        .map_err(|e| ModelBuildingError::Generic(format!("{e:?}")))?;

    typechecker::build(&subsystem_builders, parsed)
        .map_err(|e| ModelBuildingError::Generic(format!("{e:?}")))
}

pub(crate) fn create_resolved_system_from_src(
    src: &str,
) -> Result<MappedArena<ResolvedType>, ModelBuildingError> {
    let typechecked_system = create_typechecked_system_from_src(src)?;
    resolved_builder::build(&typechecked_system)
}

pub(crate) fn create_base_model_system(
    typechecked_system: &TypecheckedSystem,
) -> Result<BaseModelSystem, ModelBuildingError> {
    core_model_builder::builder::system_builder::build(typechecked_system)
}

pub(crate) fn create_postgres_core_subsystem(
    base_system: &BaseModelSystem,
    typechecked_system: &TypecheckedSystem,
) -> Result<SystemContextBuilding, ModelBuildingError> {
    let resolved_env = create_resolved_env(base_system, typechecked_system)?;

    system_builder::build(&resolved_env)
}

pub(crate) fn create_resolved_env<'a>(
    base_system: &'a BaseModelSystem,
    typechecked_system: &'a TypecheckedSystem,
) -> Result<ResolvedTypeEnv<'a>, ModelBuildingError> {
    let resolved_types = resolved_builder::build(typechecked_system)?;

    Ok(ResolvedTypeEnv {
        contexts: &base_system.contexts,
        resolved_types,
        function_definitions: &base_system.function_definitions,
    })
}
