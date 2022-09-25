use std::collections::HashMap;

use payas_core_model::mapped_arena::SerializableSlabIndex;
use payas_core_model_builder::{
    ast::ast_types::{AstExpr, LogicalOp},
    typechecker::Typed,
};
use payas_service_model::interceptor::Interceptor;

use wildmatch::WildMatch;

#[derive(Clone, Copy)]
pub enum OperationKind {
    Query,
    Mutation,
}

pub fn weave<'a>(
    operation_names: impl Iterator<Item = &'a str>,
    interceptors: &[(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)],
    operation_kind: OperationKind,
) -> HashMap<String, Vec<SerializableSlabIndex<Interceptor>>> {
    operation_names
        .map(|operation_name| {
            (
                operation_name.to_owned(),
                matching_interceptors(interceptors, operation_name, operation_kind),
            )
        })
        .collect()
}

fn matching_interceptors(
    interceptors: &[(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)],
    operation_name: &str,
    operation_kind: OperationKind,
) -> Vec<SerializableSlabIndex<Interceptor>> {
    interceptors
        .iter()
        .filter_map(|(expr, interceptor)| {
            if matches(expr, operation_name, operation_kind) {
                Some(interceptor)
            } else {
                None
            }
        })
        .cloned()
        .collect()
}

fn matches(expr: &AstExpr<Typed>, operation_name: &str, operation_kind: OperationKind) -> bool {
    match expr {
        AstExpr::FieldSelection(_) => {
            panic!("FieldSelection not supported in interceptor expression")
        }
        AstExpr::LogicalOp(logical_op) => match dbg!(logical_op) {
            LogicalOp::Not(expr, _, _) => !matches(expr, operation_name, operation_kind),
            LogicalOp::And(first, second, _, _) => {
                matches(first, operation_name, operation_kind)
                    && matches(second, operation_name, operation_kind)
            }
            LogicalOp::Or(first, second, _, _) => {
                matches(first, operation_name, operation_kind)
                    || matches(second, operation_name, operation_kind)
            }
        },
        AstExpr::RelationalOp(_) => panic!("RelationalOp not supported in interceptor expression"),
        AstExpr::StringLiteral(value, _) => matches_str(value, operation_name, operation_kind),
        AstExpr::BooleanLiteral(value, _) => *value,
        AstExpr::NumberLiteral(_, _) => {
            panic!("NumberLiteral not supported in interceptor expression")
        }
        AstExpr::StringList(_, _) => {
            panic!("List not supported in interceptor expression")
        }
    }
}

fn matches_str(expr: &str, operation_name: &str, operation_kind: OperationKind) -> bool {
    let wildmatch = WildMatch::new(expr);
    let input = match operation_kind {
        OperationKind::Query => "query",
        OperationKind::Mutation => "mutation",
    };
    wildmatch.matches(&format!("{} {}", input, operation_name))
}
