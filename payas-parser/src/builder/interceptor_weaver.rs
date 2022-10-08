use payas_core_model::{
    interceptor_kind::InterceptorKind,
    serializable_system::{InterceptionMap, InterceptionTree, InterceptorIndexWithSubsystemIndex},
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
) -> InterceptionTree {
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

/// Determine the order and nesting for interceptors.
///
/// TODO: Implement this scheme
///
/// The core idea (matches that in AspectJ):
/// - execute all the before interceptors prior to the operation, and all the after interceptors post the operation.
/// - a before interceptor defined earlier has a higher priority; it is the opposite for the after interceptors.
/// - an around interceptor defined earlier has a higher priority.
/// - all before/after interceptors defined earlier than an around interceptor execute by the time the around interceptor is executed.
///
/// Note that even for intra-service interceptors, this ides still holds true. All we need a preprocessing step to flatten the interceptors
/// to put the higher priority service's interceptors first.
///
/// Example: A service is set up with multiple interceptors in the following order (and identical
/// interceptor expressions):
///
/// ```ignore
/// @before 1
/// @before 2
/// @after  3   (has higher precedence than around 1, so must execute prior to finishing around 1)
/// @around 1
///
/// @before 3   (has higher precedence than around 2, so must execute prior to starting around 2)
/// @after  4   (has higher precedence than around 2, so must execute prior to finishing around 2)
/// @around 2
///
/// @before 4   (even when defined after around 2, must execute before the operation and after around 2 started (which has higher precedence than before 4))
///
/// @after  1   (has higher precedence than after 2, so must execute once after 2 finishes)
/// @after  2
/// ```
///
/// We want to execute the interceptors in the following order.
///
/// ```ignore
/// <before 1/>
/// <before 2/>
/// <around 1>
///     <before 3/>
///     <around 2>
///         <before 4/>
///         <OPERATION>
///         <after 4/>
///     </around 2>
///     <after 3/>
/// </around 1>
/// <after 2/>
/// <after 1/>
/// ```
///
/// Will translate to:
///
/// ```ignore
/// InterceptedOperation::Intercepted (
///     before: [
///         Interception::NonProceedingInterception(before 1)
///         Interception::NonProceedingInterception(before 2)
///     ],
///     core: Interception::ProceedingInterception(around 1, InterceptionChain(
///         Interception::NonProceedingInterception(before 3)
///         Interception::ProceedingInterception(around 2, InterceptionChain(
///             Interception::NonProceedingInterception(before 4),
///             Interception::Operation(OPERATION),
///             Interception::NonProceedingInterception(after 1)
///         )),
///         Interception::NonProceedingInterception(after 2)
///     )),
///     after: [
///         Interception::NonProceedingInterception(after 3)
///         Interception::NonProceedingInterception(after 4)
///     ]
/// )
/// ```
fn ordered(interceptions: Vec<(usize, &Interception)>) -> InterceptionTree {
    if interceptions.is_empty() {
        InterceptionTree::Plain
    } else {
        let mut before = vec![];
        let mut after = vec![];
        let mut around = vec![];

        interceptions
            .into_iter()
            .for_each(|(subsystem_index, interception)| {
                let interceptor =
                    InterceptorIndexWithSubsystemIndex::new(subsystem_index, interception.index);

                match interception.kind {
                    InterceptorKind::Before => before.push(interceptor),
                    InterceptorKind::After => after.push(interceptor),
                    InterceptorKind::Around => around.push(interceptor),
                }
            });

        let core = Box::new(InterceptionTree::Plain);

        let core = around.into_iter().fold(core, |core, interceptor| {
            Box::new(InterceptionTree::Around { core, interceptor })
        });

        InterceptionTree::Intercepted {
            before,
            core,
            after,
        }
    }
}
