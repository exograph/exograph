use core_model::mapped_arena::MappedArena;

use crate::typechecker::Type;

use super::system_builder::SystemContextBuilding;

pub(crate) fn build_primitives(types: &MappedArena<Type>, building: &mut SystemContextBuilding) {
    for (_, model_type) in types.iter() {
        if let Type::Primitive(pt) = model_type {
            let name = pt.name();

            building.primitive_types.add(&name, pt.clone());
        }
    }
}