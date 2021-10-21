use payas_model::model::{
    interceptor::Interceptor, mapped_arena::MappedArena, operation::Interceptors,
};
use serde::{de::DeserializeOwned, Serialize};
use typed_generational_arena::{IgnoreGeneration, Index};
use wildmatch::WildMatch;

use crate::{ast::ast_types::AstExpr, typechecker::Typed};

use super::{resolved_builder::ResolvedSystem, system_builder::SystemContextBuilding};

enum OperationKind {
    Query,
    Mutation,
}

pub fn weave_interceptors(resolved_system: &ResolvedSystem, building: &mut SystemContextBuilding) {
    let interceptors = &building.interceptors;
    let interceptors: Vec<(&AstExpr<Typed>, &Interceptor)> = resolved_system
        .services
        .iter()
        .flat_map(|(_, s)| {
            s.interceptors.iter().map(|i| {
                let model_interceptor = interceptors.get_by_key(&i.name).unwrap();
                (i.interceptor_kind.expr(), model_interceptor)
            })
        })
        .collect();

    weave(
        &mut building.queries,
        &interceptors,
        |o| &o.name,
        &OperationKind::Query,
        |operation, interceptors| operation.intercetors = interceptors,
    );

    weave(
        &mut building.mutations,
        &interceptors,
        |o| &o.name,
        &OperationKind::Mutation,
        |operation, interceptors| operation.intercetors = interceptors,
    );
}

fn weave<T: DeserializeOwned + Serialize>(
    operations: &mut MappedArena<T>,
    interceptors: &[(&AstExpr<Typed>, &Interceptor)],
    get_operation_name: fn(&T) -> &str,
    operation_kind: &OperationKind,
    set_interceptors: impl Fn(&mut T, Interceptors),
) {
    let weaving_info: Vec<_> =
        compute_weaving_info(operations, interceptors, get_operation_name, operation_kind);

    for (operation_id, matching_interceptors) in weaving_info.iter() {
        let operation = &mut operations[*operation_id];
        set_interceptors(operation, matching_interceptors.clone());
    }
}

fn compute_weaving_info<T: DeserializeOwned + Serialize>(
    operations: &MappedArena<T>,
    interceptors: &[(&AstExpr<Typed>, &Interceptor)],
    get_operation_name: impl Fn(&T) -> &str,
    operation_kind: &OperationKind,
) -> Vec<(Index<T, usize, IgnoreGeneration>, Interceptors)> {
    operations
        .iter()
        .map(|(operation_id, operation)| {
            let matching_interceptors = Interceptors {
                interceptors: matching_interceptors(
                    interceptors,
                    get_operation_name(operation),
                    operation_kind,
                ),
            };
            (operation_id, matching_interceptors)
        })
        .collect()
}

fn matching_interceptors(
    interceptors: &[(&AstExpr<Typed>, &Interceptor)],
    operation_name: &str,
    operation_kind: &OperationKind,
) -> Vec<Interceptor> {
    interceptors
        .iter()
        .filter_map(|(expr, interceptor)| {
            if matches_operation(expr, operation_name, operation_kind) {
                Some(*interceptor)
            } else {
                None
            }
        })
        .cloned()
        .collect()
}

fn matches_operation(
    expr: &AstExpr<Typed>,
    operation_name: &str,
    operatrion_kind: &OperationKind,
) -> bool {
    matches(expr, operation_name, operatrion_kind)
}

fn matches(expr: &AstExpr<Typed>, operation_name: &str, operatrion_kind: &OperationKind) -> bool {
    match expr {
        AstExpr::FieldSelection(_) => {
            panic!("FieldSelection not supported in interceptor expression")
        }
        AstExpr::LogicalOp(_) => todo!(),
        AstExpr::RelationalOp(_) => todo!(),
        AstExpr::StringLiteral(value, _) => matches_str(value, operation_name, operatrion_kind),
        AstExpr::BooleanLiteral(value, _) => *value,
        AstExpr::NumberLiteral(_, _) => {
            panic!("NumberLiteral not supported in interceptor expression")
        }
    }
}

fn matches_str(expr: &str, operation_name: &str, operatrion_kind: &OperationKind) -> bool {
    let wildmatch = WildMatch::new(expr);
    let input = match operatrion_kind {
        OperationKind::Query => "query",
        OperationKind::Mutation => "mutation",
    };
    wildmatch.matches(&format!("{} {}", input, operation_name))
}
