use payas_core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use payas_core_model_builder::{
    ast::ast_types::AstExpr,
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    typechecker::{typ::Type, Typed},
};
use payas_service_model::{
    interceptor::Interceptor,
    model::ModelServiceSystem,
    operation::{ServiceMutation, ServiceQuery},
    service::{Script, ServiceMethod},
    types::ServiceType,
};

use super::{
    resolved_builder, service_builder,
    type_builder::{self, ResolvedTypeEnv},
};

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub types: MappedArena<ServiceType>,

    // break this into subsystems
    pub queries: MappedArena<ServiceQuery>,

    pub mutations: MappedArena<ServiceMutation>,
    pub methods: MappedArena<ServiceMethod>,
    pub interceptors: MappedArena<Interceptor>,
    pub scripts: MappedArena<Script>,
}

impl SystemContextBuilding {
    pub fn get_id(&self, name: &str) -> Option<SerializableSlabIndex<ServiceType>> {
        self.types.get_id(name)
    }
}

pub struct ModelServiceSystemWithInterceptors {
    pub underlying: ModelServiceSystem,

    pub interceptors: Vec<(AstExpr<Typed>, Interceptor)>,
}

pub fn build(
    typechecked_system: &MappedArena<Type>,
    base_system: &BaseModelSystem,
) -> Result<ModelServiceSystemWithInterceptors, ModelBuildingError> {
    let mut building = SystemContextBuilding::default();
    let resolved_system = resolved_builder::build(&typechecked_system)?;

    let resolved_env = ResolvedTypeEnv {
        contexts: &base_system.contexts,
        resolved_types: resolved_system.service_types,
        resolved_services: resolved_system.services,
    };

    build_shallow_service(&resolved_env, &mut building);
    build_expanded_service(&resolved_env, &mut building)?;

    // let model_interceptors = building.interceptors;
    // let interceptors: Vec<(AstExpr<Typed>, Interceptor)> = resolved_system
    //     .services
    //     .values
    //     .into_iter()
    //     .flat_map(|s| {
    //         s.interceptors.into_iter().map(|resolved_interceptor| {
    //             let model_interceptor = model_interceptors
    //                 .get_by_key(&resolved_interceptor.name)
    //                 .unwrap();

    //             (
    //                 resolved_interceptor.interceptor_kind.expr().clone(),
    //                 model_interceptor.clone(),
    //             )
    //         })
    //     })
    //     .collect();

    Ok(ModelServiceSystemWithInterceptors {
        underlying: ModelServiceSystem {
            service_types: building.types.values,
            queries: building.queries,
            mutations: building.mutations,
            methods: building.methods.values,
            scripts: building.scripts.values,
            contexts: base_system.contexts.clone(),
        },
        interceptors: vec![],
    })
}

fn build_shallow_service(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    let resolved_service_types = &resolved_env.resolved_types;
    let resolved_services = &resolved_env.resolved_services;

    type_builder::build_shallow(resolved_service_types, &resolved_env.contexts, building);

    service_builder::build_shallow(resolved_service_types, resolved_services, building);
}

fn build_expanded_service(
    resolved_env: &ResolvedTypeEnv,
    building: &mut SystemContextBuilding,
) -> Result<(), ModelBuildingError> {
    let resolved_methods = &resolved_env
        .resolved_services
        .iter()
        .map(|(_, s)| s.methods.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .concat();

    type_builder::build_service_expanded(resolved_methods, resolved_env, building)?;

    service_builder::build_expanded(building);

    Ok(())
}
