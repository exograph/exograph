use payas_model::model::mapped_arena::MappedArena;

use super::{resolved_builder::ResolvedService, system_builder::SystemContextBuilding};

pub fn build_shallow(types: &MappedArena<ResolvedService>, _building: &mut SystemContextBuilding) {
    for (_, _typ) in types.iter() {
        println!("Building")
    }
}
