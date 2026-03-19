// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::{
    access::{
        AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
        CommonAccessPrimitiveExpression,
    },
    context_type::{ContextFieldType, ContextSelection},
    primitive_type::{self, PrimitiveType},
};
use core_model_builder::{
    ast::ast_types::{AstAccessExpr, AstLiteral, FieldSelection, LogicalOp, RelationalOp},
    error::ModelBuildingError,
    typechecker::Typed,
};
use subsystem_model_util::access::ModuleAccessPrimitiveExpression;

use super::type_builder::ResolvedTypeEnv;

enum PathSelection<'a> {
    Context(ContextSelection, &'a ContextFieldType),
}

pub fn compute_predicate_expression(
    expr: &AstAccessExpr<Typed>,
    resolved_env: &ResolvedTypeEnv,
) -> Result<AccessPredicateExpression<ModuleAccessPrimitiveExpression>, ModelBuildingError> {
    match expr {
        AstAccessExpr::FieldSelection(selection) => {
            match compute_selection(selection, resolved_env)? {
                PathSelection::Context(context_selection, field_type) => {
                    if field_type.innermost() == &PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE)
                    {
                        // Treat boolean context expressions in the same way as an "eq" relational expression
                        // For example, treat `AuthContext.superUser` the same way as `AuthContext.superUser == true`
                        Ok(AccessPredicateExpression::RelationalOp(
                            AccessRelationalOp::Eq(
                                Box::new(ModuleAccessPrimitiveExpression::Common(
                                    CommonAccessPrimitiveExpression::ContextSelection(
                                        context_selection,
                                    ),
                                )),
                                Box::new(ModuleAccessPrimitiveExpression::Common(
                                    CommonAccessPrimitiveExpression::BooleanLiteral(true),
                                )),
                            ),
                        ))
                    } else {
                        Err(ModelBuildingError::Generic(
                            "Context selection must be a boolean".to_string(),
                        ))
                    }
                }
            }
        }
        AstAccessExpr::LogicalOp(op) => {
            let predicate_expr =
                |expr: &AstAccessExpr<Typed>| compute_predicate_expression(expr, resolved_env);
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
        AstAccessExpr::RelationalOp(op) => {
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
                Box::new(compute_primitive_expr(left, resolved_env)?),
                Box::new(compute_primitive_expr(right, resolved_env)?),
            )))
        }
        AstAccessExpr::Literal(AstLiteral::Boolean(value, _)) => {
            Ok(AccessPredicateExpression::BooleanLiteral(*value))
        }

        _ => Err(ModelBuildingError::Generic(
            "Unsupported expression type".to_string(),
        )), // String or NumberLiteral cannot be used as a top-level expression in access rules
    }
}

fn compute_primitive_expr(
    expr: &AstAccessExpr<Typed>,
    resolved_env: &ResolvedTypeEnv,
) -> Result<ModuleAccessPrimitiveExpression, ModelBuildingError> {
    match expr {
        AstAccessExpr::FieldSelection(selection) => {
            match compute_selection(selection, resolved_env)? {
                PathSelection::Context(c, _) => Ok(ModuleAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::ContextSelection(c),
                )),
            }
        }
        AstAccessExpr::Literal(lit) => Ok(ModuleAccessPrimitiveExpression::Common(
            lit.to_common_access_primitive(),
        )),
        AstAccessExpr::LogicalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
        AstAccessExpr::RelationalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
    }
}

fn compute_selection<'a>(
    selection: &FieldSelection<Typed>,
    resolved_env: &'a ResolvedTypeEnv<'a>,
) -> Result<PathSelection<'a>, ModelBuildingError> {
    let (context_selection, column_type) =
        selection.get_context(resolved_env.contexts, resolved_env.function_definitions)?;
    Ok(PathSelection::Context(context_selection, column_type))
}
