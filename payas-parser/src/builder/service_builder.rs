use payas_model::model::{
    access::Access,
    argument::ArgumentParameter,
    mapped_arena::MappedArena,
    operation::{Mutation, MutationKind, OperationReturnType, Query, QueryKind},
    service::{Argument, ServiceMethod, ServiceMethodType},
    GqlType,
};

use super::{
    argument_builder,
    resolved_builder::{ResolvedMethod, ResolvedMethodType, ResolvedService, ResolvedType},
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

pub fn create_shallow_service(
    resolved_service: &ResolvedService,
    resolved_method: &ResolvedMethod,
    building: &mut SystemContextBuilding,
) {
    building.methods.add(
        &resolved_method.name,
        ServiceMethod {
            name: resolved_method.name.clone(),
            module_path: resolved_service.module_path.clone(),
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
                    type_id: building
                        .types
                        .get_id(arg.typ.get_underlying_typename())
                        .unwrap(),
                    modifier: arg.typ.get_modifier(),
                    is_injected: arg.is_injected,
                })
                .collect(),
            return_type: OperationReturnType {
                type_id: building
                    .types
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
    types: &MappedArena<GqlType>,
    building: &SystemContextBuilding,
) -> Query {
    let return_type = &method.return_type;

    Query {
        name: method.name.clone(),
        kind: QueryKind::Service {
            method_id: None,
            argument_param: argument_param(method, building),
        },
        return_type: OperationReturnType {
            type_id: types.get_id(return_type.get_underlying_typename()).unwrap(),
            type_name: return_type.get_underlying_typename().to_string(),
            type_modifier: return_type.get_modifier(),
        },
    }
}

fn shallow_service_mutation(
    method: &ResolvedMethod,
    types: &MappedArena<GqlType>,
    building: &SystemContextBuilding,
) -> Mutation {
    let return_type = &method.return_type;

    Mutation {
        name: method.name.clone(),
        kind: MutationKind::Service {
            method_id: None,
            argument_param: argument_param(method, building),
        },
        return_type: OperationReturnType {
            type_id: types.get_id(return_type.get_underlying_typename()).unwrap(),
            type_name: return_type.get_underlying_typename().to_string(),
            type_modifier: return_type.get_modifier(),
        },
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
                    type_name: input_name,
                    type_id: Some(input_type_id),
                    type_modifier,
                }
            } else {
                // argument must be a primitive type

                ArgumentParameter {
                    name: arg.name.clone(),
                    type_name: arg_typename.to_string(),
                    type_id: None,
                    type_modifier,
                }
            }
        })
        .collect()
}
