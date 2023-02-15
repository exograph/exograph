use core_model::mapped_arena::MappedArena;

use crate::typechecker::Type;

use super::system_builder::SystemContextBuilding;

pub(crate) fn build_primitives(types: &MappedArena<Type>, building: &mut SystemContextBuilding) {
    for (_, typ) in types.iter() {
        if let Type::Primitive(pt) = typ {
            let name = pt.name();

            building.primitive_types.add(&name, pt.clone());
        }
    }
}
