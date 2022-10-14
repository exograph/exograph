use core_model::{
    context_type::ContextFieldType, mapped_arena::MappedArena, primitive_type::PrimitiveType,
};
use core_model_builder::{
    ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp},
    error::ModelBuildingError,
    typechecker::Typed,
};
use subsystem_model_util::{
    access::{
        AccessContextSelection, AccessLogicalExpression, AccessPredicateExpression,
        AccessPrimitiveExpression, AccessRelationalOp,
    },
    types::{ServiceCompositeType, ServiceType},
};

use super::type_builder::ResolvedTypeEnv;

enum PathSelection<'a> {
    Context(AccessContextSelection, &'a ContextFieldType),
}

pub fn compute_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&ServiceCompositeType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_types: &MappedArena<ServiceType>,
) -> Result<AccessPredicateExpression, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => match compute_selection(selection, resolved_env) {
            PathSelection::Context(context_selection, field_type) => {
                if field_type.primitive_type() == &PrimitiveType::Boolean {
                    // Treat boolean context expressions in the same way as an "eq" relational expression
                    // For example, treat `AuthContext.superUser` the same way as `AuthContext.superUser == true`
                    Ok(AccessPredicateExpression::RelationalOp(
                        AccessRelationalOp::Eq(
                            Box::new(AccessPrimitiveExpression::ContextSelection(
                                context_selection,
                            )),
                            Box::new(AccessPrimitiveExpression::BooleanLiteral(true)),
                        ),
                    ))
                } else {
                    Err(ModelBuildingError::Generic(
                        "Context selection must be a boolean".to_string(),
                    ))
                }
            }
        },
        AstExpr::LogicalOp(op) => {
            let predicate_expr = |expr: &AstExpr<Typed>| {
                compute_predicate_expression(expr, self_type_info, resolved_env, subsystem_types)
            };
            Ok(match op {
                LogicalOp::And(left, right, _, _) => {
                    AccessPredicateExpression::LogicalOp(AccessLogicalExpression::And(
                        Box::new(predicate_expr(left)?),
                        Box::new(predicate_expr(right)?),
                    ))
                }
                LogicalOp::Or(left, right, _, _) => {
                    AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Or(
                        Box::new(predicate_expr(left)?),
                        Box::new(predicate_expr(right)?),
                    ))
                }
                LogicalOp::Not(value, _, _) => AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Not(Box::new(predicate_expr(value)?)),
                ),
            })
        }
        AstExpr::RelationalOp(op) => {
            let combiner = match op {
                RelationalOp::Eq(..) => AccessRelationalOp::Eq,
                RelationalOp::Neq(..) => AccessRelationalOp::Neq,
                RelationalOp::Lt(..) => AccessRelationalOp::Lt,
                RelationalOp::Lte(..) => AccessRelationalOp::Lte,
                RelationalOp::Gt(..) => AccessRelationalOp::Gt,
                RelationalOp::Gte(..) => AccessRelationalOp::Gte,
                RelationalOp::In(..) => AccessRelationalOp::In,
            };

            let (left, right) = op.sides();

            Ok(AccessPredicateExpression::RelationalOp(combiner(
                Box::new(compute_primitive_expr(left, resolved_env)),
                Box::new(compute_primitive_expr(right, resolved_env)),
            )))
        }
        AstExpr::BooleanLiteral(value, _) => Ok(AccessPredicateExpression::BooleanLiteral(*value)),

        _ => Err(ModelBuildingError::Generic(
            "Unsupported expression type".to_string(),
        )), // String or NumberLiteral cannot be used as a top-level expression in access rules
    }
}

fn compute_primitive_expr(
    expr: &AstExpr<Typed>,
    resolved_env: &ResolvedTypeEnv,
) -> AccessPrimitiveExpression {
    match expr {
        AstExpr::FieldSelection(selection) => match compute_selection(selection, resolved_env) {
            PathSelection::Context(c, _) => AccessPrimitiveExpression::ContextSelection(c),
        },
        AstExpr::StringLiteral(value, _) => AccessPrimitiveExpression::StringLiteral(value.clone()),
        AstExpr::BooleanLiteral(value, _) => AccessPrimitiveExpression::BooleanLiteral(*value),
        AstExpr::NumberLiteral(value, _) => AccessPrimitiveExpression::NumberLiteral(*value),
        AstExpr::StringList(_, _) => panic!("Access expressions do not support lists yet"),
        AstExpr::LogicalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
        AstExpr::RelationalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
    }
}

fn compute_selection<'a>(
    selection: &FieldSelection<Typed>,
    resolved_env: &'a ResolvedTypeEnv<'a>,
) -> PathSelection<'a> {
    fn flatten(selection: &FieldSelection<Typed>, acc: &mut Vec<String>) {
        match selection {
            FieldSelection::Single(identifier, _) => acc.push(identifier.0.clone()),
            FieldSelection::Select(path, identifier, _, _) => {
                flatten(path, acc);
                acc.push(identifier.0.clone());
            }
        }
    }

    fn get_context<'a>(
        path_elements: &[String],
        resolved_env: &'a ResolvedTypeEnv<'a>,
    ) -> (AccessContextSelection, &'a ContextFieldType) {
        if path_elements.len() == 2 {
            let context_type = resolved_env
                .contexts
                .values
                .iter()
                .find(|t| t.1.name == path_elements[0])
                .unwrap()
                .1;
            let field = context_type
                .fields
                .iter()
                .find(|field| field.name == path_elements[1])
                .unwrap();

            (
                AccessContextSelection::Select(
                    Box::new(AccessContextSelection::Context(path_elements[0].clone())),
                    path_elements[1].clone(),
                ),
                &field.typ,
            )
        } else {
            todo!() // Nested selection such as AuthContext.user.id
        }
    }

    let mut path_elements = vec![];
    flatten(selection, &mut path_elements);

    let (context_selection, column_type) = get_context(&path_elements, resolved_env);
    PathSelection::Context(context_selection, column_type)
}
