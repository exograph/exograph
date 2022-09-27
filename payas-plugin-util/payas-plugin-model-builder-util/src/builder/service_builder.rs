use payas_core_model::mapped_arena::{MappedArena, SerializableSlabIndex};
use payas_plugin_model_util::{
    access::Access,
    argument::{ArgumentParameter, ArgumentParameterType},
    interceptor::{Interceptor, InterceptorKind},
    operation::{OperationReturnType, ServiceMutation, ServiceQuery},
    service::{Argument, Script, ScriptKind, ServiceMethod, ServiceMethodType},
    types::ServiceType,
};

use super::{
    resolved_builder::{
        ResolvedInterceptor, ResolvedMethod, ResolvedMethodType, ResolvedService, ResolvedType,
    },
    system_builder::SystemContextBuilding,
};

pub fn build_shallow(
    _models: &MappedArena<ResolvedType>,
    services: &MappedArena<ResolvedService>,
    building: &mut SystemContextBuilding,
) {
    // create shallow service
    for (_, service) in services.iter() {
        for method in service.methods.iter() {
            create_shallow_service(service, method, building);
        }
        for interceptor in service.interceptors.iter() {
            create_shallow_interceptor(service, interceptor, building);
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (id, method) in building.methods.iter() {
        match method.operation_kind {
            ServiceMethodType::Mutation(mutation_id) => {
                let mutation = &mut building.mutations[mutation_id];
                mutation.method_id = Some(id)
            }

            ServiceMethodType::Query(query_id) => {
                let query = &mut building.queries[query_id];
                query.method_id = Some(id)
            }
        }
    }
}

fn get_or_populate_script(
    script_path: &str,
    script: &[u8],
    script_kind: ScriptKind,
    building: &mut SystemContextBuilding,
) -> SerializableSlabIndex<Script> {
    match building.scripts.get_id(script_path) {
        Some(index) => index,
        None => building.scripts.add(
            script_path,
            Script {
                path: script_path.to_owned(),
                script: script.to_owned(),
                script_kind,
            },
        ),
    }
}

fn create_shallow_service(
    resolved_service: &ResolvedService,
    resolved_method: &ResolvedMethod,
    building: &mut SystemContextBuilding,
) {
    let script = get_or_populate_script(
        &resolved_service.script_path,
        &resolved_service.script,
        ScriptKind::from_script_name(&resolved_service.script_path),
        building,
    );

    building.methods.add(
        &resolved_method.name,
        ServiceMethod {
            name: resolved_method.name.clone(),
            script,
            access: Access::restrictive(),
            operation_kind: match resolved_method.operation_kind {
                ResolvedMethodType::Query => {
                    let query = shallow_service_query(resolved_method, &building.types, building);
                    let query_id = building.queries.add(&resolved_method.name, query);
                    ServiceMethodType::Query(query_id)
                }
                ResolvedMethodType::Mutation => {
                    let mutation =
                        shallow_service_mutation(resolved_method, &building.types, building);
                    let mutation_id = building.mutations.add(&resolved_method.name, mutation);
                    ServiceMethodType::Mutation(mutation_id)
                }
            },
            is_exported: resolved_method.is_exported,
            arguments: resolved_method
                .arguments
                .iter()
                .map(|arg| Argument {
                    name: arg.name.clone(),
                    type_id: building.get_id(arg.typ.get_underlying_typename()).unwrap(),
                    modifier: arg.typ.get_modifier(),
                    is_injected: arg.is_injected,
                })
                .collect(),
            return_type: OperationReturnType {
                type_id: building
                    .get_id(resolved_method.return_type.get_underlying_typename())
                    .unwrap(),
                type_name: resolved_method
                    .return_type
                    .get_underlying_typename()
                    .to_string(),
                type_modifier: resolved_method.return_type.get_modifier(),
            },
        },
    );
}

fn shallow_service_query(
    method: &ResolvedMethod,
    service_types: &MappedArena<ServiceType>,
    building: &SystemContextBuilding,
) -> ServiceQuery {
    let return_type = &method.return_type;

    let return_type_name = return_type.get_underlying_typename();

    ServiceQuery {
        name: method.name.clone(),
        method_id: None,
        argument_param: argument_param(method, building),
        return_type: OperationReturnType {
            type_id: service_types.get_id(return_type_name).unwrap(),
            type_name: return_type_name.to_string(),
            type_modifier: return_type.get_modifier(),
        },
    }
}

fn shallow_service_mutation(
    method: &ResolvedMethod,
    service_types: &MappedArena<ServiceType>,
    building: &SystemContextBuilding,
) -> ServiceMutation {
    let return_type = &method.return_type;
    let return_type_name = return_type.get_underlying_typename();

    ServiceMutation {
        name: method.name.clone(),
        method_id: None,
        argument_param: argument_param(method, building),
        return_type: OperationReturnType {
            type_id: service_types.get_id(return_type_name).unwrap(),
            type_name: return_type_name.to_string(),
            type_modifier: return_type.get_modifier(),
        },
    }
}

// Generate parameters for the method's query or mutation.
fn argument_param(
    method: &ResolvedMethod,
    building: &SystemContextBuilding,
) -> Vec<ArgumentParameter> {
    method
        .arguments
        .iter()
        .filter(|arg| !arg.is_injected) // skip injected params!
        .map(|arg| {
            let arg_typename = arg.typ.get_underlying_typename();
            let type_modifier = arg.typ.get_modifier();
            let input_type_id = building.types.get_id(&arg_typename);

            if let Some(input_type_id) = input_type_id {
                ArgumentParameter {
                    name: arg.name.clone(),
                    typ: ArgumentParameterType {
                        name: arg_typename.to_owned(),
                        type_id: Some(input_type_id),
                        type_modifier,
                        is_primitive: false,
                    },
                }
            } else {
                // argument must be a primitive type

                ArgumentParameter {
                    name: arg.name.clone(),
                    typ: ArgumentParameterType {
                        name: arg_typename.to_string(),
                        type_id: None,
                        type_modifier,
                        is_primitive: true,
                    },
                }
            }
        })
        .collect()
}

pub fn create_shallow_interceptor(
    resolved_service: &ResolvedService,
    resolved_interceptor: &ResolvedInterceptor,
    building: &mut SystemContextBuilding,
) {
    let script = get_or_populate_script(
        &resolved_service.script_path,
        &resolved_service.script,
        ScriptKind::from_script_name(&resolved_service.script_path),
        building,
    );

    building.interceptors.add(
        &resolved_interceptor.name,
        Interceptor {
            name: resolved_interceptor.name.clone(),
            script,
            interceptor_kind: match resolved_interceptor.interceptor_kind {
                super::resolved_builder::ResolvedInterceptorKind::Before(_) => {
                    InterceptorKind::Before
                }
                super::resolved_builder::ResolvedInterceptorKind::After(_) => {
                    InterceptorKind::After
                }
                super::resolved_builder::ResolvedInterceptorKind::Around(_) => {
                    InterceptorKind::Around
                }
            },
            arguments: resolved_interceptor
                .arguments
                .iter()
                .map(|arg| Argument {
                    name: arg.name.clone(),
                    type_id: building.get_id(arg.typ.get_underlying_typename()).unwrap(),
                    modifier: arg.typ.get_modifier(),
                    is_injected: true, // implicitly set is_injected for interceptors
                })
                .collect(),
        },
    );
}
