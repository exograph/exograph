use payas_model::model::{argument::ArgumentParameterType, mapped_arena::MappedArena};

use payas_core_model_builder::builder::resolved_builder::{
    ResolvedCompositeType, ResolvedCompositeTypeKind, ResolvedType,
};

use super::system_builder::SystemContextBuilding;

pub(super) fn build_shallow(
    types: &MappedArena<ResolvedType>,
    building: &mut SystemContextBuilding,
) {
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
                        is_primitive: false,
                        actual_type_id: None,
                    },
                );
            }
        }
    }
}

pub(super) fn build_expanded(building: &mut SystemContextBuilding) {
    for (id, typ) in building.service_types.iter_mut() {
        let param_name = get_parameter_type_name(&typ.name);

        if let Some(arg_typ) = building.argument_types.get_by_key_mut(&param_name) {
            arg_typ.actual_type_id = Some(id);
            arg_typ.is_primitive = typ.is_primitive();
        }
    }
}

pub(super) fn get_parameter_type_name(method_name: &str) -> String {
    format!("{}ArgumentInput", method_name)
}
