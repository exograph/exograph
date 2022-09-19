use payas_model::model::{mapped_arena::MappedArena, GqlType, GqlTypeKind};

use crate::typechecker::Type;

use super::{
    resolved_builder::ResolvedType,
    system_builder::{BaseModelSystem, SystemContextBuilding},
};

#[derive(Debug, Clone)]
pub struct ResolvedTypeEnv<'a> {
    pub base_system: &'a BaseModelSystem,
    pub resolved_subsystem_types: &'a MappedArena<ResolvedType>,
}

impl<'a> ResolvedTypeEnv<'a> {
    pub fn get_by_key(&self, key: &str) -> Option<&ResolvedType> {
        self.resolved_subsystem_types
            .get_by_key(key)
            .or_else(|| self.base_system.resolved_primitive_types.get_by_key(key))
    }
}

pub(crate) fn build_primitives(types: &MappedArena<Type>, building: &mut SystemContextBuilding) {
    for (_, model_type) in types.iter() {
        if let Type::Primitive(pt) = model_type {
            let name = pt.name();

            let typ = GqlType {
                name: name.clone(),
                plural_name: "".to_string(),
                kind: GqlTypeKind::Primitive,
                is_input: false,
            };

            building.primitive_types.add(&name, typ);
        }
    }
}
