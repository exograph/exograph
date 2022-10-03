use payas_core_model::{
    interceptor_kind::InterceptorKind,
    serializable_system::{InterceptionMap, InterceptorIndexWithSubsystemIndex},
};
use payas_core_model_builder::{
    ast::ast_types::{AstExpr, LogicalOp},
    plugin::Interception,
    typechecker::Typed,
};

use wildmatch::WildMatch;

#[derive(Clone, Copy, Debug)]
pub enum OperationKind {
    Query,
    Mutation,
}

pub fn weave<'a>(
    operation_names: impl Iterator<Item = &'a str>,
    subsystem_interceptions: &[(usize, Vec<Interception>)],
    operation_kind: OperationKind,
) -> InterceptionMap {
    InterceptionMap {
        map: operation_names
            .map(|operation_name| {
                (
                    operation_name.to_owned(),
                    matching_interceptors(subsystem_interceptions, operation_name, operation_kind),
                )
            })
            .collect(),
    }
}

fn matching_interceptors(
    subsystem_interceptions: &[(usize, Vec<Interception>)],
    operation_name: &str,
    operation_kind: OperationKind,
) -> Vec<InterceptorIndexWithSubsystemIndex> {
    let matching_interceptions: Vec<(usize, &Interception)> = subsystem_interceptions
        .iter()
        .flat_map(|(subsystem_index, interceptions)| {
            interceptions.iter().flat_map(|interception| {
                if matches(&interception.expr, operation_name, operation_kind) {
                    Some((*subsystem_index, interception))
                } else {
                    None
                }
            })
        })
        .collect();

    ordered(matching_interceptions)
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

fn ordered(interceptions: Vec<(usize, &Interception)>) -> Vec<InterceptorIndexWithSubsystemIndex> {
    let mut processed = Vec::new();
    let mut deferred = Vec::new();

    for (subsystem_index, interception) in interceptions {
        let interceptor =
            InterceptorIndexWithSubsystemIndex::new(subsystem_index, interception.index.clone());
        if interception.kind == InterceptorKind::Before {
            processed.push(interceptor);
        } else {
            deferred.push(interceptor);
        }
    }
    processed.extend(deferred.into_iter());
    processed
}
