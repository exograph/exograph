use payas_core_model::mapped_arena::MappedArena;
use payas_core_plugin::{interceptor_kind::InterceptorKind, serializable_system::InterceptorIndex};

use crate::{
    ast::ast_types::AstExpr,
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    typechecker::{Type, Typed},
};

pub trait SubsystemBuilder {
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Option<Result<SubsystemBuild, ModelBuildingError>>;
}

pub struct SubsystemBuild {
    pub id: String,
    pub serialized_subsystem: Vec<u8>,
    pub query_names: Vec<String>,
    pub mutation_names: Vec<String>,
    pub interceptions: Vec<Interception>,
}

#[derive(Debug)]
pub struct Interception {
    pub expr: AstExpr<Typed>,
    pub kind: InterceptorKind,
    pub index: InterceptorIndex,
}
