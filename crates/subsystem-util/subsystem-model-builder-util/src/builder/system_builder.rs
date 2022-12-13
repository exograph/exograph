use std::path::PathBuf;

use core_model::mapped_arena::{MappedArena, SerializableSlab, SerializableSlabIndex};
use core_model_builder::{
    ast::ast_types::{AstExpr, AstService},
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    typechecker::{typ::TypecheckedSystem, Typed},
};
use subsystem_model_util::{
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

#[derive(Debug)]
pub struct SystemContextBuilding {
    pub types: MappedArena<ServiceType>,

    // break this into subsystems
    pub queries: MappedArena<ServiceQuery>,

    pub mutations: MappedArena<ServiceMutation>,
    pub methods: MappedArena<ServiceMethod>,
    pub interceptors: SerializableSlab<Interceptor>, // Don't use MappedArena because we use a composite key (service name + method name) here
    pub scripts: MappedArena<Script>,
}

impl Default for SystemContextBuilding {
    fn default() -> Self {
        Self {
            types: MappedArena::default(),
            queries: MappedArena::default(),
            mutations: MappedArena::default(),
            methods: MappedArena::default(),
            interceptors: SerializableSlab::new(),
            scripts: MappedArena::default(),
        }
    }
}

impl SystemContextBuilding {
    pub fn get_id(&self, name: &str) -> Option<SerializableSlabIndex<ServiceType>> {
        self.types.get_id(name)
    }
}

pub struct ModelServiceSystemWithInterceptors {
    pub underlying: ModelServiceSystem,

    pub interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)>,
}

/// Builds a [ModelServiceSystemWithInterceptors], with a subset of [AstService]s chosen by closure.
///  
/// `service_selection_closure` - A closure that will return `Some(name)` for each [AstService] the
///                               subsystem supports, where `name` is the annotation name of the plugin
///                               annotation (e.g. `"deno"` for `@deno`).
/// `process_script` - A closure that will process a script at the provided [PathBuf] into a runnable form for usage
///                    during subsystem resolution at runtime.
pub fn build_with_selection(
    typechecked_system: &TypecheckedSystem,
    base_system: &BaseModelSystem,
    service_selection_closure: impl Fn(&AstService<Typed>) -> Option<String>,
    process_script: impl Fn(&AstService<Typed>, &PathBuf) -> Result<Vec<u8>, ModelBuildingError>,
) -> Result<ModelServiceSystemWithInterceptors, ModelBuildingError> {
    let mut building = SystemContextBuilding::default();
    let resolved_system = resolved_builder::build(
        typechecked_system,
        service_selection_closure,
        process_script,
    )?;

    let resolved_env = ResolvedTypeEnv {
        contexts: &base_system.contexts,
        resolved_types: resolved_system.service_types,
        resolved_services: resolved_system.services,
    };

    build_shallow_service(&resolved_env, &mut building);
    build_expanded_service(&resolved_env, &mut building)?;

    let interceptors: Vec<(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)> = resolved_env
        .resolved_services
        .values
        .iter()
        .flat_map(|(_, resolved_service)| {
            resolved_service
                .interceptors
                .iter()
                .map(|resolved_interceptor| {
                    let model_interceptor = building
                        .interceptors
                        .iter()
                        .find_map(|(index, i)| {
                            (i.service_name == resolved_interceptor.service_name
                                && i.method_name == resolved_interceptor.method_name)
                                .then_some(index)
                        })
                        .unwrap();

                    (
                        resolved_interceptor.interceptor_kind.expr().clone(),
                        model_interceptor,
                    )
                })
        })
        .collect();

    Ok(ModelServiceSystemWithInterceptors {
        underlying: ModelServiceSystem {
            service_types: building.types.values,
            queries: building.queries,
            mutations: building.mutations,
            methods: building.methods.values,
            scripts: building.scripts.values,
            contexts: base_system.contexts.clone(),
            interceptors: building.interceptors,
        },
        interceptors,
    })
}

fn build_shallow_service(resolved_env: &ResolvedTypeEnv, building: &mut SystemContextBuilding) {
    let resolved_service_types = &resolved_env.resolved_types;
    let resolved_services = &resolved_env.resolved_services;

    type_builder::build_shallow(resolved_service_types, resolved_env.contexts, building);

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
