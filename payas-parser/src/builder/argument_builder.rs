use payas_model::model::{argument::ArgumentParameterType, mapped_arena::MappedArena};

use crate::builder::resolved_builder::ResolvedCompositeTypeKind;

use super::{
    resolved_builder::{ResolvedCompositeType, ResolvedType},
    system_builder::SystemContextBuilding,
};

pub fn build_shallow(types: &MappedArena<ResolvedType>, building: &mut SystemContextBuilding) {
    // build an argument type for each composite type
    // (we need an input object for each composite type argument)
    for (_, typ) in types.iter() {
        if let ResolvedType::Composite(ResolvedCompositeType {
            kind: ResolvedCompositeTypeKind::NonPersistent { is_input },
            ..
        }) = typ
        {
            if *is_input {
                let param_name = get_parameter_type_name(&typ.name());

                building.argument_types.add(
                    &param_name,
                    ArgumentParameterType {
                        name: param_name.clone(),
                        actual_type_id: None,
                    },
                );
            }
        }
    }
}

pub fn build_expanded(building: &mut SystemContextBuilding) {
    for (id, typ) in building.types.iter_mut() {
        let param_name = get_parameter_type_name(&typ.name);

        if let Some(arg_typ) = building.argument_types.get_by_key_mut(&param_name) {
            arg_typ.actual_type_id = Some(id);
        }
    }
}

pub fn get_parameter_type_name(method_name: &str) -> String {
    format!("{}ArgumentInput", method_name)
}
