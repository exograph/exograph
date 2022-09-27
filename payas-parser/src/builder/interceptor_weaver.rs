use std::collections::HashMap;

use payas_core_model::mapped_arena::{SerializableSlab, SerializableSlabIndex};
use payas_core_model_builder::{
    ast::ast_types::{AstExpr, LogicalOp},
    typechecker::Typed,
};
use payas_deno_model::interceptor::{Interceptor, InterceptorKind};

use wildmatch::WildMatch;

#[derive(Clone, Copy)]
pub enum OperationKind {
    Query,
    Mutation,
}

pub fn weave<'a>(
    operation_names: impl Iterator<Item = &'a str>,
    expr_interceptor_pairs: &[(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)],
    interceptors: &SerializableSlab<Interceptor>,
    operation_kind: OperationKind,
) -> HashMap<String, Vec<SerializableSlabIndex<Interceptor>>> {
    operation_names
        .map(|operation_name| {
            (
                operation_name.to_owned(),
                matching_interceptors(
                    expr_interceptor_pairs,
                    interceptors,
                    operation_name,
                    operation_kind,
                ),
            )
        })
        .collect()
}

fn matching_interceptors(
    expr_interceptor_pairs: &[(AstExpr<Typed>, SerializableSlabIndex<Interceptor>)],
    interceptors: &SerializableSlab<Interceptor>,
    operation_name: &str,
    operation_kind: OperationKind,
) -> Vec<SerializableSlabIndex<Interceptor>> {
    ordered(
        expr_interceptor_pairs
            .iter()
            .filter_map(|(expr, interceptor)| {
                if matches(expr, operation_name, operation_kind) {
                    Some(interceptor)
                } else {
                    None
                }
            })
            .cloned()
            .collect(),
        interceptors,
    )
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

pub fn ordered(
    interceptor_indices: Vec<SerializableSlabIndex<Interceptor>>,
    interceptors: &SerializableSlab<Interceptor>,
) -> Vec<SerializableSlabIndex<Interceptor>> {
    let mut processed = Vec::new();
    let mut deferred = Vec::new();

    for interceptor_index in interceptor_indices {
        let interceptor = &interceptors[interceptor_index];
        if interceptor.interceptor_kind == InterceptorKind::Before {
            processed.push(interceptor_index);
        } else {
            deferred.push(interceptor_index);
        }
    }
    processed.extend(deferred.into_iter());
    processed
}
