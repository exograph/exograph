use core_model::{
    mapped_arena::{MappedArena, SerializableSlabIndex},
    types::{BaseOperationReturnType, FieldType, Named},
};
use core_plugin_shared::interception::InterceptorKind;
use subsystem_model_util::{
    access::Access,
    argument::{ArgumentParameter, ArgumentParameterType},
    interceptor::Interceptor,
    operation::{ServiceMutation, ServiceQuery},
    service::{Argument, Script, ServiceMethod, ServiceMethodType},
    types::ServiceType,
};

use crate::builder::resolved_builder::ResolvedFieldType;

use super::{
    resolved_builder::{
        ResolvedInterceptor, ResolvedInterceptorKind, ResolvedMethod, ResolvedMethodType,
        ResolvedService, ResolvedType,
    },
    system_builder::SystemContextBuilding,
};

pub fn build_shallow(
    _types: &MappedArena<ResolvedType>,
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
    building: &mut SystemContextBuilding,
) -> SerializableSlabIndex<Script> {
    match building.scripts.get_id(script_path) {
        Some(index) => index,
        None => building.scripts.add(
            script_path,
            Script {
                path: script_path.to_owned(),
                script: script.to_owned(),
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
                    type_id: arg.typ.wrap(building.get_id(arg.typ.name()).unwrap()),
                    is_injected: arg.is_injected,
                })
                .collect(),
            return_type: {
                let plain_return_type = BaseOperationReturnType {
                    associated_type_id: building
                        .get_id(resolved_method.return_type.name())
                        .unwrap(),
                    type_name: resolved_method.return_type.name().to_string(),
                };

                resolved_method.return_type.wrap(plain_return_type)
            },
        },
    );
}

fn shallow_service_query(
    method: &ResolvedMethod,
    service_types: &MappedArena<ServiceType>,
    building: &SystemContextBuilding,
) -> ServiceQuery {
    let resolved_return_type = &method.return_type;
    let return_type_name = resolved_return_type.name();

    ServiceQuery {
        name: method.name.clone(),
        method_id: None,
        argument_param: argument_param(method, building),
        return_type: {
            let plain_return_type = BaseOperationReturnType {
                associated_type_id: service_types.get_id(return_type_name).unwrap(),
                type_name: return_type_name.to_string(),
            };
            resolved_return_type.wrap(plain_return_type)
        },
    }
}

fn shallow_service_mutation(
    method: &ResolvedMethod,
    service_types: &MappedArena<ServiceType>,
    building: &SystemContextBuilding,
) -> ServiceMutation {
    let resolved_return_type = &method.return_type;
    let return_type_name = resolved_return_type.name();

    ServiceMutation {
        name: method.name.clone(),
        method_id: None,
        argument_param: argument_param(method, building),
        return_type: {
            let plain_return_type = BaseOperationReturnType {
                associated_type_id: service_types.get_id(return_type_name).unwrap(),
                type_name: return_type_name.to_string(),
            };
            resolved_return_type.wrap(plain_return_type)
        },
    }
}

// Generate parameters for the method's query or mutation.
fn argument_param(
    method: &ResolvedMethod,
    building: &SystemContextBuilding,
) -> Vec<ArgumentParameter> {
    fn to_field_type(
        typename: &str,
        type_id: Option<SerializableSlabIndex<ServiceType>>,
        arg_typ: &FieldType<ResolvedFieldType>,
    ) -> FieldType<ArgumentParameterType> {
        let base_field_type = ArgumentParameterType {
            name: typename.to_owned(),
            type_id,
            is_primitive: type_id.is_none(),
        };
        arg_typ.wrap(base_field_type)
    }
    method
        .arguments
        .iter()
        .filter(|arg| !arg.is_injected) // skip injected params!
        .map(|arg| {
            let arg_typename = arg.typ.name();
            let input_type_id = building.types.get_id(arg_typename);

            let field_typ = to_field_type(arg_typename, input_type_id, &arg.typ);

            ArgumentParameter {
                name: arg.name.clone(),
                typ: field_typ,
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
        building,
    );

    building.interceptors.insert(Interceptor {
        service_name: resolved_service.name.clone(),
        method_name: resolved_interceptor.method_name.clone(),
        script,
        interceptor_kind: match resolved_interceptor.interceptor_kind {
            ResolvedInterceptorKind::Before(_) => InterceptorKind::Before,
            ResolvedInterceptorKind::After(_) => InterceptorKind::After,
            ResolvedInterceptorKind::Around(_) => InterceptorKind::Around,
        },
        arguments: resolved_interceptor
            .arguments
            .iter()
            .map(|arg| Argument {
                name: arg.name.clone(),
                type_id: arg.typ.wrap(building.get_id(arg.typ.name()).unwrap()),
                is_injected: true, // implicitly set is_injected for interceptors
            })
            .collect(),
    });
}
