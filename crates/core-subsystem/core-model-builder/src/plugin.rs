use core_plugin_shared::interception::{InterceptorIndex, InterceptorKind};

use crate::{ast::ast_types::AstExpr, typechecker::Typed};
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
