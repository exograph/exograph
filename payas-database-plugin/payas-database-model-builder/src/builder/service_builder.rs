use payas_model::model::{
    access::Access,
    argument::{ArgumentParameter, ArgumentParameterTypeWithModifier},
    interceptor::{Interceptor, InterceptorKind},
    mapped_arena::{MappedArena, SerializableSlabIndex},
    operation::{Interceptors, Mutation, MutationKind, OperationReturnType, Query, QueryKind},
    service::{Argument, Script, ScriptKind, ServiceMethod, ServiceMethodType},
    GqlType,
};

use super::{
    argument_builder,
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
                if let MutationKind::Service { method_id, .. } = &mut mutation.kind {
                    *method_id = Some(id)
                } else {
                    panic!()
                }
            }

            ServiceMethodType::Query(query_id) => {
                let query = &mut building.queries[query_id];
                if let QueryKind::Service { method_id, .. } = &mut query.kind {
                    *method_id = Some(id)
                } else {
                    panic!()
                }
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

pub fn create_shallow_service(
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
                    let query = shallow_service_query(
                        resolved_method,
                        &building.primitive_types,
                        &building.service_types,
                        building,
                    );
                    let query_id = building.queries.add(&resolved_method.name, query);
                    ServiceMethodType::Query(query_id)
                }
                ResolvedMethodType::Mutation => {
                    let mutation = shallow_service_mutation(
                        resolved_method,
                        &building.primitive_types,
                        &building.service_types,
                        building,
                    );
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
                    is_primitive: arg.typ.is_underlying_type_primitive(),
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
                is_primitive: resolved_method.return_type.is_underlying_type_primitive(),
                type_modifier: resolved_method.return_type.get_modifier(),
                is_persistent: false,
            },
        },
    );
}

fn shallow_service_query(
    method: &ResolvedMethod,
    primitive_types: &MappedArena<GqlType>,
    service_types: &MappedArena<GqlType>,
    building: &SystemContextBuilding,
) -> Query {
    let return_type = &method.return_type;

    let return_type_name = return_type.get_underlying_typename();

    Query {
        name: method.name.clone(),
        kind: QueryKind::Service {
            method_id: None,
            argument_param: argument_param(method, building),
        },
        return_type: OperationReturnType {
            type_id: primitive_types
                .get_id(return_type_name)
                .unwrap_or_else(|| service_types.get_id(return_type_name).unwrap()),
            type_name: return_type_name.to_string(),
            is_primitive: return_type.is_underlying_type_primitive(),
            type_modifier: return_type.get_modifier(),
            is_persistent: false,
        },
        interceptors: Interceptors::default(),
    }
}

fn shallow_service_mutation(
    method: &ResolvedMethod,
    primitive_types: &MappedArena<GqlType>,
    service_types: &MappedArena<GqlType>,
    building: &SystemContextBuilding,
) -> Mutation {
    let return_type = &method.return_type;
    let return_type_name = return_type.get_underlying_typename();

    Mutation {
        name: method.name.clone(),
        kind: MutationKind::Service {
            method_id: None,
            argument_param: argument_param(method, building),
        },
        return_type: OperationReturnType {
            type_id: primitive_types
                .get_id(return_type_name)
                .unwrap_or_else(|| service_types.get_id(return_type_name).unwrap()),
            type_name: return_type_name.to_string(),
            is_primitive: return_type.is_underlying_type_primitive(),
            type_modifier: return_type.get_modifier(),
            is_persistent: false,
        },
        interceptors: Interceptors::default(),
    }
}

// Generate parameters for the method's query or mutation.
fn argument_param(
    method: &ResolvedMethod,
    building: &SystemContextBuilding,
) -> Vec<ArgumentParameter> {
    let arg_types = &building.argument_types;

    method
        .arguments
        .iter()
        .filter(|arg| !arg.is_injected) // skip injected params!
        .map(|arg| {
            let arg_typename = arg.typ.get_underlying_typename();
            let type_modifier = arg.typ.get_modifier();
            let input_name = argument_builder::get_parameter_type_name(arg_typename);
            let input_type_id = arg_types.get_id(&input_name);

            if let Some(input_type_id) = input_type_id {
                ArgumentParameter {
                    name: arg.name.clone(),
                    typ: ArgumentParameterTypeWithModifier {
                        type_name: input_name,
                        type_id: Some(input_type_id),
                        type_modifier,
                    },
                }
            } else {
                // argument must be a primitive type

                ArgumentParameter {
                    name: arg.name.clone(),
                    typ: ArgumentParameterTypeWithModifier {
                        type_name: arg_typename.to_string(),
                        type_id: None,
                        type_modifier,
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
                    is_primitive: arg.typ.is_underlying_type_primitive(),
                    modifier: arg.typ.get_modifier(),
                    is_injected: true, // implicitly set is_injected for interceptors
                })
                .collect(),
        },
    );
}
