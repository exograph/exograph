// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::access::{
    AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
};
use core_plugin_interface::core_model_builder::{
    ast::ast_types::{AstExpr, LogicalOp, RelationalOp},
    error::ModelBuildingError,
    typechecker::Typed,
};

pub(super) fn compute_logical_op<PrimExpr: Send + Sync>(
    op: &LogicalOp<Typed>,
    predicate_expr: impl Fn(
        &AstExpr<Typed>,
    ) -> Result<AccessPredicateExpression<PrimExpr>, ModelBuildingError>,
) -> Result<AccessPredicateExpression<PrimExpr>, ModelBuildingError> {
    Ok(match op {
        LogicalOp::And(left, right, _, _) => {
            let left_expr = predicate_expr(left)?;
            let right_expr = predicate_expr(right)?;

            AccessPredicateExpression::LogicalOp(AccessLogicalExpression::And(
                Box::new(left_expr),
                Box::new(right_expr),
            ))
        }
        LogicalOp::Or(left, right, _, _) => {
            let left_expr = predicate_expr(left)?;
            let right_expr = predicate_expr(right)?;

            AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Or(
                Box::new(left_expr),
                Box::new(right_expr),
            ))
        }
        LogicalOp::Not(value, _, _) => {
            let expr = predicate_expr(value)?;

            AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Not(Box::new(expr)))
        }
    })
}

pub(super) fn compute_relational_op<PrimExpr: Send + Sync>(
    op: &RelationalOp<Typed>,
    primitive_expr: impl Fn(&AstExpr<Typed>) -> Result<PrimExpr, ModelBuildingError>,
) -> Result<AccessPredicateExpression<PrimExpr>, ModelBuildingError> {
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

    let left_expr = primitive_expr(left)?;
    let right_expr = primitive_expr(right)?;

    Ok(AccessPredicateExpression::RelationalOp(combiner(
        Box::new(left_expr),
        Box::new(right_expr),
    )))
}
