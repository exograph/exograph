use payas_model::model::{mapped_arena::MappedArena, ContextType, GqlType};

use crate::{error::ModelBuildingError, typechecker::Type};

use super::{
    context_builder,
    resolved_builder::{self, ResolvedType},
    type_builder,
};

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub primitive_types: MappedArena<GqlType>,
    pub contexts: MappedArena<ContextType>,
    pub context_types: MappedArena<GqlType>, // The GqlType version of ContextType to pass in as injected parameter (TODO: Is there a better way to do this?)
}

#[derive(Debug)]
pub struct BaseModelSystem {
    pub resolved_primitive_types: MappedArena<ResolvedType>,
    pub primitive_types: MappedArena<GqlType>,
    pub contexts: MappedArena<ContextType>,
    pub context_types: MappedArena<GqlType>, // The GqlType version of ContextType to pass in as injected parameter (TODO: Is there a better way to do this?)
}

pub fn build(types: &MappedArena<Type>) -> Result<BaseModelSystem, ModelBuildingError> {
    let mut building = SystemContextBuilding::default();

    type_builder::build_primitives(&types, &mut building);

    let resolved = resolved_builder::build(types)?;

    context_builder::build(&resolved.contexts, &mut building);

    Ok(BaseModelSystem {
        resolved_primitive_types: resolved.primitive_types,
        primitive_types: building.primitive_types,
        contexts: building.contexts,
        context_types: building.context_types,
    })
}
