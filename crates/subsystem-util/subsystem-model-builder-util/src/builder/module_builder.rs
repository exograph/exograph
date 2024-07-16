// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::{
    mapped_arena::{MappedArena, SerializableSlabIndex},
    types::{BaseOperationReturnType, FieldType, Named},
};
use core_plugin_shared::interception::InterceptorKind;
use subsystem_model_util::{
    access::Access,
    argument::{ArgumentParameter, ArgumentParameterType},
    interceptor::Interceptor,
    module::{Argument, ModuleMethod, ModuleMethodType, Script},
    operation::{ModuleMutation, ModuleQuery},
    types::{ForeignModuleType, ModuleOperationReturnType, ModuleType},
};

use crate::builder::resolved_builder::ResolvedFieldType;

use super::{
    resolved_builder::{
        ResolvedInterceptor, ResolvedInterceptorKind, ResolvedMethod, ResolvedMethodType,
        ResolvedModule, ResolvedType,
    },
    system_builder::SystemContextBuilding,
};

pub fn build_shallow(
    _types: &MappedArena<ResolvedType>,
    modules: &MappedArena<ResolvedModule>,
    building: &mut SystemContextBuilding,
) {
    // create shallow module
    for (_, module) in modules.iter() {
        for method in module.methods.iter() {
            create_shallow_module(module, method, building);
        }
        for interceptor in module.interceptors.iter() {
            create_shallow_interceptor(module, interceptor, building);
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (id, method) in building.methods.iter() {
        match method.operation_kind {
            ModuleMethodType::Mutation(mutation_id) => {
                let mutation = &mut building.mutations[mutation_id];
                mutation.method_id = Some(id)
            }

            ModuleMethodType::Query(query_id) => {
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

fn create_shallow_module(
    resolved_module: &ResolvedModule,
    resolved_method: &ResolvedMethod,
    building: &mut SystemContextBuilding,
) {
    let script = get_or_populate_script(
        &resolved_module.script_path,
        &resolved_module.script,
        building,
    );

    building.methods.add(
        &resolved_method.name,
        ModuleMethod {
            name: resolved_method.name.clone(),
            script,
            access: Access::restrictive(),
            operation_kind: match resolved_method.operation_kind {
                ResolvedMethodType::Query => {
                    let query = shallow_module_query(resolved_method, &building.types, building);
                    let query_id = building.queries.add(&resolved_method.name, query);
                    ModuleMethodType::Query(query_id)
                }
                ResolvedMethodType::Mutation => {
                    let mutation =
                        shallow_module_mutation(resolved_method, &building.types, building);
                    let mutation_id = building.mutations.add(&resolved_method.name, mutation);
                    ModuleMethodType::Mutation(mutation_id)
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
                match &resolved_method.return_type.innermost().module_name {
                    Some(module_name) => {
                        let plain_return_type = ForeignModuleType {
                            module_name: module_name.clone(),
                            return_type_name: resolved_method.return_type.name().to_string(),
                        };
                        ModuleOperationReturnType::Foreign(
                            resolved_method.return_type.wrap(plain_return_type),
                        )
                    }
                    None => {
                        let plain_return_type = BaseOperationReturnType {
                            associated_type_id: building
                                .get_id(resolved_method.return_type.name())
                                .unwrap(),
                            type_name: resolved_method.return_type.name().to_string(),
                        };

                        ModuleOperationReturnType::Own(
                            resolved_method.return_type.wrap(plain_return_type),
                        )
                    }
                }
            },
        },
    );
}

fn shallow_module_query(
    method: &ResolvedMethod,
    module_types: &MappedArena<ModuleType>,
    building: &SystemContextBuilding,
) -> ModuleQuery {
    let resolved_return_type = &method.return_type;
    let return_type_name = resolved_return_type.name();

    let module_name = resolved_return_type.innermost().module_name.clone();

    let return_type = match module_name {
        Some(module_name) => {
            let plain_return_type = ForeignModuleType {
                module_name,
                return_type_name: return_type_name.to_string(),
            };
            ModuleOperationReturnType::Foreign(resolved_return_type.wrap(plain_return_type))
        }

        None => {
            let plain_return_type = BaseOperationReturnType {
                associated_type_id: module_types.get_id(return_type_name).unwrap(),
                type_name: return_type_name.to_string(),
            };
            ModuleOperationReturnType::Own(resolved_return_type.wrap(plain_return_type))
        }
    };

    ModuleQuery {
        name: method.name.clone(),
        method_id: None,
        argument_param: argument_param(method, building),
        return_type,
    }
}

fn shallow_module_mutation(
    method: &ResolvedMethod,
    module_types: &MappedArena<ModuleType>,
    building: &SystemContextBuilding,
) -> ModuleMutation {
    let resolved_return_type = &method.return_type;
    let return_type_name = resolved_return_type.name();

    ModuleMutation {
        name: method.name.clone(),
        method_id: None,
        argument_param: argument_param(method, building),
        return_type: {
            let plain_return_type = BaseOperationReturnType {
                associated_type_id: module_types.get_id(return_type_name).unwrap(),
                type_name: return_type_name.to_string(),
            };
            ModuleOperationReturnType::Own(resolved_return_type.wrap(plain_return_type))
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
        type_id: Option<SerializableSlabIndex<ModuleType>>,
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
    resolved_module: &ResolvedModule,
    resolved_interceptor: &ResolvedInterceptor,
    building: &mut SystemContextBuilding,
) {
    let script = get_or_populate_script(
        &resolved_module.script_path,
        &resolved_module.script,
        building,
    );

    building.interceptors.insert(Interceptor {
        module_name: resolved_module.name.clone(),
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
