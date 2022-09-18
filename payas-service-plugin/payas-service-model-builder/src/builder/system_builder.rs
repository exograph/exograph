use payas_core_model_builder::{
    ast::ast_types::AstExpr,
    builder::{
        resolved_builder::ResolvedType, system_builder::BaseModelSystem,
        type_builder::ResolvedTypeEnv,
    },
    error::ModelBuildingError,
    typechecker::{typ::Type, Typed},
};
use payas_model::model::{
    argument::ArgumentParameterType,
    interceptor::Interceptor,
    mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex},
    operation::{Mutation, Query},
    service::{Script, ServiceMethod},
    GqlType,
};

use super::{
    argument_builder,
    resolved_builder::{self, ResolvedServiceSystem},
    service_builder, type_builder,
};

pub struct ModelServiceSystem {
    pub service_types: SerializableSlab<GqlType>,

    // query related
    pub argument_types: SerializableSlab<ArgumentParameterType>,
    pub queries: MappedArena<Query>,

    // mutation related
    pub mutations: MappedArena<Mutation>,

    // service related
    pub methods: SerializableSlab<ServiceMethod>,
    pub scripts: SerializableSlab<Script>,

    pub interceptors: Vec<(AstExpr<Typed>, Interceptor)>,
}

pub fn build(
    typechecked_system: &MappedArena<Type>,
    base_system: &BaseModelSystem,
) -> Result<ModelServiceSystem, ModelBuildingError> {
    let mut building = SystemContextBuilding::default();

    let resolved_system = resolved_builder::build(&typechecked_system)?;

    let mut resolved_primitive_types = MappedArena::default();

    base_system.primitive_types.iter().for_each(|(_, typ)| {
        resolved_primitive_types.add(
            typ.name.as_str(),
            ResolvedType::Primitive(
                payas_core_model_builder::typechecker::typ::PrimitiveType::from_str(
                    typ.name.as_str(),
                ),
            ),
        );
    });

    let resolved_env = ResolvedTypeEnv {
        base_system,
        resolved_subsystem_types: &resolved_system.service_types,
    };

    build_shallow_service(&resolved_system, &resolved_env, &mut building);
    build_expanded_service(&resolved_system, &resolved_env, &mut building)?;

    let model_interceptors = building.interceptors;
    let interceptors: Vec<(AstExpr<Typed>, Interceptor)> = resolved_system
        .services
        .values
        .into_iter()
        .flat_map(|s| {
            s.interceptors.into_iter().map(|resolved_interceptor| {
                let model_interceptor = model_interceptors
                    .get_by_key(&resolved_interceptor.name)
                    .unwrap();

                (
                    resolved_interceptor.interceptor_kind.expr().clone(),
                    model_interceptor.clone(),
                )
            })
        })
        .collect();

    Ok(ModelServiceSystem {
        service_types: building.service_types.values,
        argument_types: building.argument_types.values,
        queries: building.queries,
        mutations: building.mutations,
        methods: building.methods.values,
        scripts: building.scripts.values,
        interceptors,
    })
}

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    // TODO: Break this up into deno/wasm
    pub service_types: MappedArena<GqlType>,

    pub argument_types: MappedArena<ArgumentParameterType>,

    // break this into subsystems
    pub queries: MappedArena<Query>,

    pub mutation_types: MappedArena<GqlType>,
    pub mutations: MappedArena<Mutation>,
    pub methods: MappedArena<ServiceMethod>,
    pub interceptors: MappedArena<Interceptor>,
    pub scripts: MappedArena<Script>,
}

impl SystemContextBuilding {
    pub fn get_id(
        &self,
        name: &str,
        resolved_env: &ResolvedTypeEnv,
    ) -> Option<SerializableSlabIndex<GqlType>> {
        resolved_env
            .base_system
            .primitive_types
            .get_id(name)
            .or_else(|| self.service_types.get_id(name))
            .or_else(|| resolved_env.base_system.context_types.get_id(name))
    }
}

fn build_shallow_service(
    resolved_system: &ResolvedServiceSystem,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) {
    let resolved_service_types = &resolved_system.service_types;
    let resolved_services = &resolved_system.services;

    type_builder::build_shallow(resolved_service_types, building);

    argument_builder::build_shallow(resolved_service_types, building);

    service_builder::build_shallow(
        resolved_service_types,
        resolved_services,
        resolved_env,
        building,
    );
}

fn build_expanded_service(
    resolved_system: &ResolvedServiceSystem,
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let resolved_methods = &resolved_system
        .services
        .iter()
        .map(|(_, s)| s.methods.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .concat();

    type_builder::build_service_expanded(resolved_methods, resolved_env, building)?;

    argument_builder::build_expanded(building);

    service_builder::build_expanded(building);

    Ok(())
}
