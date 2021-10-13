use payas_model::model::{argument::{ArgumentParameter, ArgumentParameterType}, mapped_arena::MappedArena};

use super::{resolved_builder::{ResolvedCompositeType, ResolvedMethod, ResolvedService, ResolvedType}, system_builder::SystemContextBuilding};

pub fn build_shallow(typ: &MappedArena<ResolvedService>, building: &mut SystemContextBuilding) {
    for (_, service) in typ.iter() {
        for method  in service.methods.iter() {
            // create a type for each method 
            let param_type_name = get_parameter_type_name(&method.name);
            let typ = ArgumentParameterType {
                name: param_type_name.clone(),
                arguments: vec![],
            };

            building.argument_types.add(&param_type_name, typ);
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (_, method) in building.methods.iter_mut() {
        let param_type_name = get_parameter_type_name(&method.name);
        let arg_type = building.argument_types.get_by_key_mut(&param_type_name).unwrap();

        for (arg_name, type_id, modifier) in &method.arguments {
            let typ = &building.types[*type_id];

            arg_type.arguments.push(
                ArgumentParameter {
                    name: arg_name.clone(),
                    type_name: typ.name.clone(),
                    type_modifier: modifier.clone(),
                    type_id: *type_id,
                }
            )
        }
    }

}

pub fn get_parameter_type_name(method_name: &str) -> String {
    format!("{}MethodInput", method_name)
}