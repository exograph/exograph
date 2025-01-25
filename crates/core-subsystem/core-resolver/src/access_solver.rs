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
use core_model::access::{
    AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
    CommonAccessPrimitiveExpression,
};
use thiserror::Error;

use common::context::{ContextExtractionError, RequestContext};
use common::value::Val;

use crate::{context_extractor::ContextExtractor, number_cmp::NumberWrapper};

/// Access predicate that can be logically combined with other predicates.
pub trait AccessPredicate<'a>:
    From<bool> + std::ops::Not<Output = Self> + 'a + Send + Sync
{
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;
}

#[derive(Error, Debug)]
pub enum AccessSolverError {
    #[error("{0}")]
    ContextExtraction(#[from] ContextExtractionError),

    #[error("{0}")]
    Generic(Box<dyn std::error::Error + Send + Sync>),

    #[error("{0}")]
    AccessInputContextPathElement(#[from] AccessInputContextPathElementError),
}

#[derive(Error, Debug)]
pub enum AccessInputContextPathElementError {
    #[error("Index cannot be used on an object: {0}")]
    IndexOnObject(String),

    #[error("Property key cannot be used on a list: {0}")]
    PropertyOnList(String),
}

#[derive(Debug)]
pub struct AccessInputContext<'a> {
    pub value: &'a Val,
    pub ignore_missing_context: bool,
    pub aliases: HashMap<&'a str, AccessInputContextPath<'a>>,
}

#[derive(Clone)]
pub enum AccessInputContextPathElement<'a> {
    Property(&'a str),
    Index(usize),
}

impl<'a> std::fmt::Debug for AccessInputContextPathElement<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessInputContextPathElement::Property(s) => write!(f, "{}", s),
            AccessInputContextPathElement::Index(i) => write!(f, "[{}]", i),
        }
    }
}

#[derive(Clone)]
pub struct AccessInputContextPath<'a>(pub Vec<AccessInputContextPathElement<'a>>);

impl<'a> AccessInputContextPath<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &AccessInputContextPathElement<'a>> {
        self.0.iter()
    }
}

impl<'a> std::ops::Index<usize> for AccessInputContextPath<'a> {
    type Output = AccessInputContextPathElement<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<'a> std::fmt::Debug for AccessInputContextPath<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 && matches!(e, AccessInputContextPathElement::Property(_)) {
                write!(f, ".")?;
            }
            write!(f, "{:?}", e)?;
        }
        Ok(())
    }
}

impl<'a> AccessInputContext<'a> {
    pub fn resolve(
        &self,
        path: AccessInputContextPath<'a>,
    ) -> Result<Option<&'a Val>, AccessInputContextPathElementError> {
        fn _resolve<'a>(
            val: Option<&'a Val>,
            path: &AccessInputContextPath<'a>,
        ) -> Result<Option<&'a Val>, AccessInputContextPathElementError> {
            let mut current = val;
            for part in path.iter() {
                match current {
                    Some(Val::Object(map)) => match part {
                        AccessInputContextPathElement::Property(key) => {
                            current = map.get(*key);
                        }
                        AccessInputContextPathElement::Index(_) => {
                            return Err(AccessInputContextPathElementError::IndexOnObject(
                                format!("{:?}", &path),
                            ));
                        }
                    },
                    Some(Val::List(list)) => match part {
                        AccessInputContextPathElement::Property(_) => {
                            return Err(AccessInputContextPathElementError::PropertyOnList(
                                format!("{:?}", &path),
                            ));
                        }
                        AccessInputContextPathElement::Index(index) => {
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
                    AccessInputContextPathElement::Property(key) => {
                        let alias_path = self.aliases.get(key); // "a" -> ["articles"]

                        match alias_path {
                            Some(alias_path) => {
                                let alias_root_value = _resolve(Some(self.value), alias_path)?;
                                _resolve(alias_root_value, &AccessInputContextPath(rest.to_vec()))
                                // For expression a.title, the path will be ["title"]
                            }
                            None => _resolve(Some(self.value), &path),
                        }
                    }
                    AccessInputContextPathElement::Index(_) => Err(
                        AccessInputContextPathElementError::IndexOnObject(format!("{:?}", &path)),
                    ),
                }
            }
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
    Res: AccessPredicate<'a> + std::fmt::Debug,
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
        input_context: Option<&AccessInputContext<'a>>,
        expr: &AccessPredicateExpression<PrimExpr>,
    ) -> Result<Option<Res>, AccessSolverError> {
        match expr {
            AccessPredicateExpression::LogicalOp(op) => {
                self.solve_logical_op(request_context, input_context, op)
                    .await
            }
            AccessPredicateExpression::RelationalOp(op) => {
                self.solve_relational_op(request_context, input_context, op)
                    .await
            }
            AccessPredicateExpression::BooleanLiteral(value) => Ok(Some((*value).into())),
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
        input_context: Option<&AccessInputContext<'a>>,
        op: &AccessRelationalOp<PrimExpr>,
    ) -> Result<Option<Res>, AccessSolverError>;

    /// Solve logical operations such as `not`, `and`, `or`.
    async fn solve_logical_op(
        &self,
        request_context: &RequestContext<'a>,
        input_context: Option<&AccessInputContext<'a>>,
        op: &AccessLogicalExpression<PrimExpr>,
    ) -> Result<Option<Res>, AccessSolverError> {
        Ok(match op {
            AccessLogicalExpression::Not(underlying) => {
                let underlying_predicate = self
                    .solve(request_context, input_context, underlying)
                    .await?;
                underlying_predicate.map(|p| p.not())
            }
            AccessLogicalExpression::And(left, right) => {
                let left_predicate = self.solve(request_context, input_context, left).await?;
                let right_predicate = self.solve(request_context, input_context, right).await?;

                match (left_predicate, right_predicate) {
                    (Some(left_predicate), Some(right_predicate)) => {
                        Some(left_predicate.and(right_predicate))
                    }
                    _ => None,
                }
            }
            AccessLogicalExpression::Or(left, right) => {
                let left_predicate = self.solve(request_context, input_context, left).await?;
                let right_predicate = self.solve(request_context, input_context, right).await?;

                match (left_predicate, right_predicate) {
                    (Some(left_predicate), Some(right_predicate)) => {
                        Some(left_predicate.or(right_predicate))
                    }
                    (Some(left_predicate), None) => Some(left_predicate),
                    (None, Some(right_predicate)) => Some(right_predicate),
                    (None, None) => None,
                }
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
        CommonAccessPrimitiveExpression::NumberLiteral(value) => Some(Val::Number((*value).into())),
        CommonAccessPrimitiveExpression::NullLiteral => Some(Val::Null),
    })
}

pub fn eq_values(left_value: &Val, right_value: &Val) -> bool {
    match (left_value, right_value) {
        (Val::Number(left_number), Val::Number(right_number)) => {
            // We have a more general implementation of `PartialEq` for `Val` that accounts for
            // different number types. So, we use that implementation here instead of using just `==`
            NumberWrapper(left_number.clone()).partial_cmp(&NumberWrapper(right_number.clone()))
                == Some(std::cmp::Ordering::Equal)
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
            NumberWrapper(left_number.clone()) < NumberWrapper(right_number.clone())
        }
        _ => unreachable!("The operands of `<` operator must be numbers"),
    }
}

pub fn lte_values(left_value: &Val, right_value: &Val) -> bool {
    match (left_value, right_value) {
        (Val::Number(left_number), Val::Number(right_number)) => {
            NumberWrapper(left_number.clone()) <= NumberWrapper(right_number.clone())
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
    fn test_access_input_context_path() {
        let input_context = AccessInputContext {
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
            ignore_missing_context: false,
            aliases: HashMap::from([(
                "a",
                AccessInputContextPath(vec![
                    AccessInputContextPathElement::Property("articles"),
                    AccessInputContextPathElement::Index(0),
                ]),
            )]),
        };

        let existing_value = input_context
            .resolve(AccessInputContextPath(vec![
                AccessInputContextPathElement::Property("a"),
                AccessInputContextPathElement::Property("title"),
            ]))
            .unwrap();
        assert_eq!(Some(&json!("Article 1").into()), existing_value);

        let non_existing_value = input_context
            .resolve(AccessInputContextPath(vec![
                AccessInputContextPathElement::Property("a"),
                AccessInputContextPathElement::Property("author"),
            ]))
            .unwrap();
        assert_eq!(None, non_existing_value);

        let non_existing_alias = input_context
            .resolve(AccessInputContextPath(vec![
                AccessInputContextPathElement::Property("b"),
                AccessInputContextPathElement::Property("title"),
            ]))
            .unwrap();
        assert_eq!(None, non_existing_alias);
    }
}
