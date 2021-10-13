use std::path::PathBuf;

use payas_model::model::{GqlType, argument::{self, ArgumentParameter}, mapped_arena::MappedArena, operation::{Mutation, MutationKind, OperationReturnType, Query, QueryKind}, service::{ServiceMethod, ServiceMethodType}};

use super::{argument_builder, resolved_builder::{ResolvedMethod, ResolvedMethodType, ResolvedService, ResolvedType}, system_builder::SystemContextBuilding};

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

pub fn create_shallow_service(
    resolved_service: &ResolvedService,
    resolved_method: &ResolvedMethod,
    building: &mut SystemContextBuilding,
) {
    building.methods.add(
        &resolved_method.name,
        ServiceMethod {
            name: resolved_method.name.clone(),
            module_path: PathBuf::from(resolved_service.module_path.clone()),
            operation_kind: match resolved_method.operation_kind {
                ResolvedMethodType::Query => {
                    let query = shallow_service_query(resolved_method, &building.types, &building);
                    let query_id = building.queries.add(&resolved_method.name, query);
                    ServiceMethodType::Query(query_id)
                }
                ResolvedMethodType::Mutation => {
                    let mutation = shallow_service_mutation(resolved_method, &building.types, &building);
                    let mutation_id = building.mutations.add(&resolved_method.name, mutation);
                    ServiceMethodType::Mutation(mutation_id)
                }
            },
            arguments: resolved_method.arguments.iter()
                .map(|arg| 
                    (arg.name.clone(), building.types.get_id(&arg.typ.get_underlying_typename()).unwrap().clone(), arg.typ.get_modifier().clone()))
                .collect(),
            return_type: None, // FIXME
        },
    );
}

pub fn build_expanded(_building: &mut SystemContextBuilding) {
}

fn shallow_service_query(method: &ResolvedMethod, types: &MappedArena<GqlType>, building: &SystemContextBuilding) -> Query {
    let return_type = method.return_type.as_ref().unwrap();

    Query {
        name: method.name.clone(),
        kind: QueryKind::Service(argument_param(method, building)),
        return_type: OperationReturnType {
            type_id: types.get_id(return_type.get_underlying_typename()).unwrap(),
            type_name: return_type.get_underlying_typename().to_string(),
            type_modifier: return_type.get_modifier(),
        },
    }
}

fn shallow_service_mutation(method: &ResolvedMethod, types: &MappedArena<GqlType>, building: &SystemContextBuilding) -> Mutation {
    Mutation {
        name: method.name.clone(),
        kind: MutationKind::Service(argument_param(method, building)),
        return_type: method
            .return_type
            .as_ref()
            .map(|return_type| OperationReturnType {
                type_id: types.get_id(return_type.get_underlying_typename()).unwrap(),
                type_name: return_type.get_underlying_typename().to_string(),
                type_modifier: return_type.get_modifier(),
            }),
    }
}

fn argument_param(
    method: &ResolvedMethod,
    building: &SystemContextBuilding
) -> Vec<ArgumentParameter> {
    let type_name = argument_builder::get_parameter_type_name(&method.name);
    let type_id = building.argument_types.get_id(&type_name).unwrap();
    let typ = &building.argument_types[type_id];
    typ.arguments.clone()
}
