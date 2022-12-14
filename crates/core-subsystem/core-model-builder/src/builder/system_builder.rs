use core_model::primitive_type::PrimitiveType;
use core_model::{context_type::ContextType, mapped_arena::MappedArena};

use crate::error::ModelBuildingError;
use crate::typechecker::typ::TypecheckedSystem;

use super::{context_builder, resolved_builder, type_builder};

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub primitive_types: MappedArena<PrimitiveType>,
    pub contexts: MappedArena<ContextType>,
}

#[derive(Debug)]
pub struct BaseModelSystem {
    pub primitive_types: MappedArena<PrimitiveType>,
    pub contexts: MappedArena<ContextType>,
}

pub fn build(
    typechecked_system: &TypecheckedSystem,
) -> Result<BaseModelSystem, ModelBuildingError> {
    let mut building = SystemContextBuilding {
        primitive_types: MappedArena::default(),
        contexts: MappedArena::default(),
    };

    type_builder::build_primitives(&typechecked_system.types, &mut building);

    let resolved = resolved_builder::build(&typechecked_system.types)?;

    context_builder::build(&resolved.contexts, &mut building);

    Ok(BaseModelSystem {
        primitive_types: building.primitive_types,
        contexts: building.contexts,
    })
}
