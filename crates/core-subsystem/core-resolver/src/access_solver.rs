// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_trait::async_trait;
use common::value::val::ValNumber;
use core_model::access::{
    AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
    CommonAccessPrimitiveExpression,
};
use thiserror::Error;

use common::context::{ContextExtractionError, RequestContext};
use common::value::Val;

use crate::context_extractor::ContextExtractor;

/// Access predicate that can be logically combined with other predicates.
pub trait AccessPredicate: From<bool> + std::ops::Not<Output = Self> + Send + Sync {
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;

    fn is_true(&self) -> bool;
    fn is_false(&self) -> bool;
}

#[derive(Error, Debug)]
pub enum AccessSolverError {
    #[error("{0}")]
    ContextExtraction(#[from] ContextExtractionError),

    #[error("{0}")]
    Generic(Box<dyn std::error::Error + Send + Sync>),

    #[error("{0}")]
    AccessInputPathElement(#[from] AccessInputPathElementError),
}

#[derive(Error, Debug)]
pub enum AccessInputPathElementError {
    #[error("Index cannot be used on an object: {0}")]
    IndexOnObject(String),

    #[error("Property key cannot be used on a list: {0}")]
    PropertyOnList(String),
}

#[derive(Debug)]
pub struct AccessInput<'a> {
    pub value: &'a Val,
    pub ignore_missing_value: bool,
    pub aliases: HashMap<&'a str, AccessInputPath<'a>>,
}

#[derive(Clone)]
pub enum AccessInputPathElement<'a> {
    Property(&'a str),
    Index(usize),
}

impl std::fmt::Debug for AccessInputPathElement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessInputPathElement::Property(s) => write!(f, "{}", s),
            AccessInputPathElement::Index(i) => write!(f, "[{}]", i),
        }
    }
}

#[derive(Clone)]
pub struct AccessInputPath<'a>(pub Vec<AccessInputPathElement<'a>>);

impl<'a> AccessInputPath<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &AccessInputPathElement<'a>> {
        self.0.iter()
    }
}

impl<'a> std::ops::Index<usize> for AccessInputPath<'a> {
    type Output = AccessInputPathElement<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::fmt::Debug for AccessInputPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 && matches!(e, AccessInputPathElement::Property(_)) {
                write!(f, ".")?;
            }
            write!(f, "{:?}", e)?;
        }
        Ok(())
    }
}

impl<'a> AccessInput<'a> {
    pub fn resolve(
        &self,
        path: AccessInputPath<'a>,
    ) -> Result<Option<&'a Val>, AccessInputPathElementError> {
        fn _resolve<'a>(
            val: Option<&'a Val>,
            path: &AccessInputPath<'a>,
        ) -> Result<Option<&'a Val>, AccessInputPathElementError> {
            let mut current = val;
            for part in path.iter() {
                match current {
                    Some(Val::Object(map)) => match part {
                        AccessInputPathElement::Property(key) => {
                            current = map.get(*key);
                        }
                        AccessInputPathElement::Index(_) => {
                            return Err(AccessInputPathElementError::IndexOnObject(format!(
                                "{:?}",
                                &path
                            )));
                        }
                    },
                    Some(Val::List(list)) => match part {
                        AccessInputPathElement::Property(_) => {
                            return Err(AccessInputPathElementError::PropertyOnList(format!(
                                "{:?}",
                                &path
                            )));
                        }
                        AccessInputPathElement::Index(index) => {
                            current = list.get(*index);
                        }
                    },
                    _ => return Ok(None),
                }
            }
            Ok(current)
        }

        match path.0.as_slice() {
            [] => Ok(Some(self.value)), // "self"
            [key, rest @ ..] => {
                match key {
                    AccessInputPathElement::Property(key) => {
                        let alias_path = self.aliases.get(key); // "a" -> ["articles"]

                        match alias_path {
                            Some(alias_path) => {
                                let alias_root_value = _resolve(Some(self.value), alias_path)?;
                                _resolve(alias_root_value, &AccessInputPath(rest.to_vec()))
                                // For expression a.title, the path will be ["title"]
                            }
                            None => _resolve(Some(self.value), &path),
                        }
                    }
                    AccessInputPathElement::Index(_) => Err(
                        AccessInputPathElementError::IndexOnObject(format!("{:?}", &path)),
                    ),
                }
            }
        }
    }
}

pub enum AccessSolution<Res> {
    Solved(Res),
    Unsolvable(Res), // the attribute indicates that if forced to resolve, what it should be
}

impl<Res> std::fmt::Debug for AccessSolution<Res>
where
    Res: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessSolution::Solved(res) => write!(f, "Solved({:?})", res),
            AccessSolution::Unsolvable(res) => write!(f, "NotSolved({:?})", res),
        }
    }
}

impl<Res> AccessSolution<Res>
where
    Res: std::fmt::Debug,
{
    pub fn map<U>(self, f: impl FnOnce(Res) -> U) -> AccessSolution<U> {
        match self {
            AccessSolution::Solved(res) => AccessSolution::Solved(f(res)),
            AccessSolution::Unsolvable(res) => AccessSolution::Unsolvable(f(res)),
        }
    }

    pub fn resolve(self) -> Res {
        match self {
            AccessSolution::Solved(res) => res,
            AccessSolution::Unsolvable(res) => res,
        }
    }
}

impl<Res> AccessSolution<Res>
where
    Res: AccessPredicate + std::fmt::Debug,
{
    fn not(self) -> Self {
        match self {
            AccessSolution::Solved(res) => AccessSolution::Solved(res.not()),
            AccessSolution::Unsolvable(res) => AccessSolution::Unsolvable(res),
        }
    }

    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (AccessSolution::Solved(left_predicate), AccessSolution::Solved(right_predicate)) => {
                AccessSolution::Solved(left_predicate.and(right_predicate))
            }
            (
                AccessSolution::Solved(left_predicate),
                AccessSolution::Unsolvable(right_predicate),
            )
            | (
                AccessSolution::Unsolvable(left_predicate),
                AccessSolution::Solved(right_predicate),
            ) => AccessSolution::Solved(left_predicate.and(right_predicate)),
            (
                AccessSolution::Unsolvable(left_predicate),
                AccessSolution::Unsolvable(right_predicate),
            ) => AccessSolution::Unsolvable(left_predicate.and(right_predicate)),
        }
    }

    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (AccessSolution::Solved(left_predicate), AccessSolution::Solved(right_predicate)) => {
                AccessSolution::Solved(left_predicate.or(right_predicate))
            }
            (
                AccessSolution::Solved(left_predicate),
                AccessSolution::Unsolvable(right_predicate),
            )
            | (
                AccessSolution::Unsolvable(left_predicate),
                AccessSolution::Solved(right_predicate),
            ) => AccessSolution::Solved(left_predicate.or(right_predicate)),
            (
                AccessSolution::Unsolvable(left_predicate),
                AccessSolution::Unsolvable(right_predicate),
            ) => AccessSolution::Unsolvable(left_predicate.or(right_predicate)),
        }
    }
}

/// Solve access control logic.
///
/// Typically, the user of this trait will use the `solve` method.
///
/// ## Parameters:
/// - `PrimExpr`: Primitive expression type
/// - `Res`: Result predicate type
#[async_trait]
pub trait AccessSolver<'a, PrimExpr, Res>
where
    PrimExpr: Send + Sync + std::fmt::Debug,
    Res: AccessPredicate + std::fmt::Debug,
{
    /// Solve access control logic.
    ///
    /// Typically, this method (through the implementation of `and`, `or`, `not` as well as
    /// `solve_relational_op`) tries to produce the simplest possible predicate given the request
    /// context. For example, `AuthContext.id == 1` will produce true or false depending on the
    /// value of `AuthContext.id` in the request context. However, `AuthContext.id == 1 &&
    /// self.published` might produce a residue `self.published` if the `AuthContext.id` is 1. This
    /// scheme allows the implementor to optimize to avoid passing a filter to the downstream data
    /// source as well as return a "Not authorized" error when possible (instead of an empty/null
    /// result).
    async fn solve(
        &self,
        request_context: &RequestContext<'a>,
        input_value: Option<&AccessInput<'a>>,
        expr: &AccessPredicateExpression<PrimExpr>,
    ) -> Result<AccessSolution<Res>, AccessSolverError> {
        match expr {
            AccessPredicateExpression::LogicalOp(op) => {
                self.solve_logical_op(request_context, input_value, op)
                    .await
            }
            AccessPredicateExpression::RelationalOp(op) => {
                self.solve_relational_op(request_context, input_value, op)
                    .await
            }
            AccessPredicateExpression::BooleanLiteral(value) => {
                Ok(AccessSolution::Solved((*value).into()))
            }
        }
    }

    /// Solve relational operation such as `=`, `!=`, `<`, `>`, `<=`, `>=`.
    ///
    /// Since relating two primitive expressions depend on the subsystem, this method is abstract.
    /// For example, a database subsystem produce a relational expression comparing two columns
    /// such as `column_a < column_b`.
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        input_value: Option<&AccessInput<'a>>,
        op: &AccessRelationalOp<PrimExpr>,
    ) -> Result<AccessSolution<Res>, AccessSolverError>;

    /// Solve logical operations such as `not`, `and`, `or`.
    async fn solve_logical_op(
        &self,
        request_context: &RequestContext<'a>,
        input_value: Option<&AccessInput<'a>>,
        op: &AccessLogicalExpression<PrimExpr>,
    ) -> Result<AccessSolution<Res>, AccessSolverError> {
        Ok(match op {
            AccessLogicalExpression::Not(underlying) => {
                let underlying_predicate =
                    self.solve(request_context, input_value, underlying).await?;
                underlying_predicate.not()
            }
            AccessLogicalExpression::And(left, right) => {
                let left_predicate = self.solve(request_context, input_value, left).await?;

                // Short-circuit if the left predicate is false
                if matches!(&left_predicate, AccessSolution::Solved(res) if res.is_false()) {
                    return Ok(left_predicate);
                }

                let right_predicate = self.solve(request_context, input_value, right).await?;

                left_predicate.and(right_predicate)
            }
            AccessLogicalExpression::Or(left, right) => {
                let left_predicate = self.solve(request_context, input_value, left).await?;

                // Short-circuit if the left predicate is true
                if matches!(&left_predicate, AccessSolution::Solved(res) if res.is_true()) {
                    return Ok(left_predicate);
                }

                let right_predicate = self.solve(request_context, input_value, right).await?;

                left_predicate.or(right_predicate)
            }
        })
    }
}

/// A primitive expression that has been reduced to a JSON value or an unresolved context
pub async fn reduce_common_primitive_expression<'a>(
    context_extractor: &(impl ContextExtractor + Send + Sync),
    request_context: &RequestContext<'a>,
    expr: &'a CommonAccessPrimitiveExpression,
) -> Result<Option<Val>, AccessSolverError> {
    Ok(match expr {
        CommonAccessPrimitiveExpression::ContextSelection(selection) => context_extractor
            .extract_context_selection(request_context, selection)
            .await?
            .cloned(),
        CommonAccessPrimitiveExpression::StringLiteral(value) => Some(Val::String(value.clone())),
        CommonAccessPrimitiveExpression::BooleanLiteral(value) => Some(Val::Bool(*value)),
        CommonAccessPrimitiveExpression::NumberLiteral(value) => {
            Some(Val::Number(ValNumber::I64(*value)))
        }
        CommonAccessPrimitiveExpression::NullLiteral => Some(Val::Null),
    })
}

pub fn eq_values(left_value: &Val, right_value: &Val) -> bool {
    match (left_value, right_value) {
        (Val::Number(left_number), Val::Number(right_number)) => {
            // We have a more general implementation of `PartialEq` for `Val` that accounts for
            // different number types. So, we use that implementation here instead of using just `==`
            left_number.clone() == right_number.clone()
        }
        _ => left_value == right_value,
    }
}

pub fn neq_values(left_value: &Val, right_value: &Val) -> bool {
    !eq_values(left_value, right_value)
}

pub fn in_values(left_value: &Val, right_value: &Val) -> bool {
    match right_value {
        Val::List(values) => values.contains(left_value),
        _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
    }
}

pub fn lt_values(left_value: &Val, right_value: &Val) -> bool {
    match (left_value, right_value) {
        (Val::Number(left_number), Val::Number(right_number)) => {
            left_number.clone() < right_number.clone()
        }
        _ => unreachable!("The operands of `<` operator must be numbers"),
    }
}

pub fn lte_values(left_value: &Val, right_value: &Val) -> bool {
    match (left_value, right_value) {
        (Val::Number(left_number), Val::Number(right_number)) => {
            left_number.clone() <= right_number.clone()
        }
        _ => unreachable!("The operands of `<=` operator must be numbers"),
    }
}

pub fn gt_values(left_value: &Val, right_value: &Val) -> bool {
    !lte_values(left_value, right_value)
}

pub fn gte_values(left_value: &Val, right_value: &Val) -> bool {
    !lt_values(left_value, right_value)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_access_input_path() {
        let input_value = AccessInput {
            value: &json!({
                "name": "John",
                "articles": [
                    {
                        "title": "Article 1",
                    },
                    {
                        "title": "Article 2",
                    }
                ]
            })
            .into(),
            ignore_missing_value: false,
            aliases: HashMap::from([(
                "a",
                AccessInputPath(vec![
                    AccessInputPathElement::Property("articles"),
                    AccessInputPathElement::Index(0),
                ]),
            )]),
        };

        let existing_value = input_value
            .resolve(AccessInputPath(vec![
                AccessInputPathElement::Property("a"),
                AccessInputPathElement::Property("title"),
            ]))
            .unwrap();
        assert_eq!(Some(&json!("Article 1").into()), existing_value);

        let non_existing_value = input_value
            .resolve(AccessInputPath(vec![
                AccessInputPathElement::Property("a"),
                AccessInputPathElement::Property("author"),
            ]))
            .unwrap();
        assert_eq!(None, non_existing_value);

        let non_existing_alias = input_value
            .resolve(AccessInputPath(vec![
                AccessInputPathElement::Property("b"),
                AccessInputPathElement::Property("title"),
            ]))
            .unwrap();
        assert_eq!(None, non_existing_alias);
    }
}
