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
pub trait AccessPredicate: From<bool> + std::ops::Not<Output = Self> + Clone + Send + Sync {
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
    pub missing_value_policy: MissingValuePolicy<'a>,
    pub aliases: HashMap<&'a str, AccessInputPath<'a>>,
}

/// What a value that an access expression refers to, but the input doesn't supply, means
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingValuePolicy<'a> {
    /// Skip the check (for updates, where an absent field keeps its current value, which the
    /// database predicate checks)
    Ignore,
    /// Check against the database row where possible, fail otherwise
    Evaluate,
    /// For creates, where an absent value will be stored as null, so it is evaluated as null (the
    /// way SQL evaluates a null column)
    Create {
        /// In a nested create, the field referring to the parent. For example, in
        /// `createUser(data: {name: "u", profile: {bio: "b"}})`, the profile input has no `user`:
        /// it is filled in from the inserted user. Its value is unknown until that insert, so
        /// checks through this field are skipped. `None` for a top-level create.
        parent_reference: Option<&'a str>,
    },
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

/// The result of solving an access expression for a request as far as it allows.
///
/// In some cases, the solution gives a definitive verdict for each entity (`self`, such as a row in
/// Postgres). For example, with `AuthContext.id` as 1, `AuthContext.id == 1 && self.published`
/// reduces to the residue `self.published` (a predicate left for later evaluation, such as a
/// database filter), and `AuthContext.id == 2 && self.published` reduces to `false`. Either result
/// is [`AccessSolution::Solved`].
///
/// In other cases, a comparison remains undecided:
/// - **Unknown**: a comparison with a value that doesn't exist, which is neither true nor false.
///   For example, for an anonymous request, `AuthContext.id == self.ownerId` compares a missing
///   context value, whether in a query's database filter, a module's access rule, or a mutation's
///   precheck. Similarly, in a Postgres create's precheck, when the input omits `ownerId` (and
///   `ownerId` doesn't have a default value), `self.ownerId == 5` compares a null column. An
///   unknown comparison doesn't allow access: a filter excludes the entities for which the rule is
///   unknown, and other checks deny the request.
/// - **Skipped**: a comparison that a precheck (which checks a mutation's input) leaves to another
///   check. Other checks never skip a comparison. For example, for an update whose input omits
///   `title`, the precheck skips `self.title == "draft"`: the field keeps its current value, which
///   another check covers (in Postgres, the database filter on the rows to update). So a skipped
///   comparison must not deny access, even under a negation, and the precheck assumes whichever
///   value allows access. With `&&`, the rest of the expression decides: `<skipped> && e` allows
///   where `e` does. With `||`, the skipped comparison alone may satisfy the rule, so
///   `<skipped> || e` allows everywhere. For instance, with the rule
///   `self.title == "draft" || self.published` and the input `{published: false}`, the update is
///   legitimate only if the current title is "draft", so the precheck allows it and leaves the
///   title to the other check. Reducing the rule to `self.published` would deny every such update,
///   even of a draft.
///
/// An expression with such a comparison is [`AccessSolution::Unsolvable`]. Following three-valued
/// logic, it tracks both where the expression is true and where it is false (it is unknown
/// elsewhere):
/// - `Solved(p)` is just `p`.
/// - **Unknown** is never true or false.
/// - **Skipped** is both true and false everywhere, so each operator takes whichever allows
///   access.
/// - `!e` is true where `e` is false, and false where `e` is true.
/// - `l && r` is true where both are true, and false where either is false.
/// - `l || r` is true where either is true, and false where both are false.
///
/// Combining with a solved result that decides the expression (`false` for `&&`, `true` for
/// `||`) gives that result. When forced to [`resolve`](AccessSolution::resolve), an unsolvable
/// expression allows access only where it is true.
///
/// Tracking only where an expression is true isn't enough, since a negation needs where its
/// operand is false. For example, for an _anonymous_ request:
///
/// ```text
/// expression                                             true where        false where
/// AuthContext.id == self.ownerId                         nowhere           nowhere
/// AuthContext.id == self.ownerId || self.published       self.published    nowhere
/// !(AuthContext.id == self.ownerId || self.published)    nowhere           self.published
/// ```
///
/// So the rule allows no entities. Knowing only that the `||` is true for published entities, the
/// negation would have to either keep `self.published` (allowing the entities that the rule
/// excludes) or negate it (allowing the unpublished entities, for which the rule is unknown).
pub enum AccessSolution<Res> {
    /// The expression is just the predicate
    Solved(Res),
    /// The expression depends on an unknown or skipped comparison
    Unsolvable {
        /// Where the expression is true (what it resolves to)
        true_when: Res,
        /// Where the expression is false
        false_when: Res,
    },
}

impl<Res> std::fmt::Debug for AccessSolution<Res>
where
    Res: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessSolution::Solved(res) => write!(f, "Solved({:?})", res),
            AccessSolution::Unsolvable {
                true_when,
                false_when,
            } => write!(
                f,
                "Unsolvable {{ true_when: {:?}, false_when: {:?} }}",
                true_when, false_when
            ),
        }
    }
}

impl<Res> AccessSolution<Res>
where
    Res: std::fmt::Debug,
{
    pub fn map<U>(self, f: impl Fn(Res) -> U) -> AccessSolution<U> {
        match self {
            AccessSolution::Solved(res) => AccessSolution::Solved(f(res)),
            AccessSolution::Unsolvable {
                true_when,
                false_when,
            } => AccessSolution::Unsolvable {
                true_when: f(true_when),
                false_when: f(false_when),
            },
        }
    }

    /// The predicate for where the expression is true, which is where it allows access
    pub fn resolve(self) -> Res {
        match self {
            AccessSolution::Solved(res) => res,
            AccessSolution::Unsolvable { true_when, .. } => true_when,
        }
    }
}

impl<Res> AccessSolution<Res>
where
    Res: AccessPredicate + std::fmt::Debug,
{
    /// A comparison that is neither true nor false, such as one with a missing context value
    pub fn unknown() -> Self {
        AccessSolution::Unsolvable {
            true_when: false.into(),
            false_when: false.into(),
        }
    }

    /// A comparison that a precheck leaves to another check, so it doesn't deny access
    pub fn skipped() -> Self {
        AccessSolution::Unsolvable {
            true_when: true.into(),
            false_when: true.into(),
        }
    }

    fn not(self) -> Self {
        match self {
            AccessSolution::Solved(res) => AccessSolution::Solved(res.not()),
            AccessSolution::Unsolvable {
                true_when,
                false_when,
            } => AccessSolution::Unsolvable {
                true_when: false_when,
                false_when: true_when,
            },
        }
    }

    /// Combines two solutions with `and` (see [`AccessSolution`])
    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (AccessSolution::Solved(left_predicate), AccessSolution::Solved(right_predicate)) => {
                AccessSolution::Solved(left_predicate.and(right_predicate))
            }
            // A `false` decides the result
            (AccessSolution::Solved(predicate), _) | (_, AccessSolution::Solved(predicate))
                if predicate.is_false() =>
            {
                AccessSolution::Solved(predicate)
            }
            (left, right) => {
                let (left_true_when, left_false_when) = left.true_and_false_when();
                let (right_true_when, right_false_when) = right.true_and_false_when();

                AccessSolution::Unsolvable {
                    true_when: left_true_when.and(right_true_when),
                    false_when: left_false_when.or(right_false_when),
                }
            }
        }
    }

    /// Combines two solutions with `or` (see [`AccessSolution`])
    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (AccessSolution::Solved(left_predicate), AccessSolution::Solved(right_predicate)) => {
                AccessSolution::Solved(left_predicate.or(right_predicate))
            }
            // A `true` decides the result
            (AccessSolution::Solved(predicate), _) | (_, AccessSolution::Solved(predicate))
                if predicate.is_true() =>
            {
                AccessSolution::Solved(predicate)
            }
            (left, right) => {
                let (left_true_when, left_false_when) = left.true_and_false_when();
                let (right_true_when, right_false_when) = right.true_and_false_when();

                AccessSolution::Unsolvable {
                    true_when: left_true_when.or(right_true_when),
                    false_when: left_false_when.and(right_false_when),
                }
            }
        }
    }

    /// Where the expression is true and where it is false
    fn true_and_false_when(self) -> (Res, Res) {
        match self {
            AccessSolution::Solved(res) => (res.clone(), res.not()),
            AccessSolution::Unsolvable {
                true_when,
                false_when,
            } => (true_when, false_when),
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
            if let Ok(number) = value.parse::<i64>() {
                Some(Val::Number(ValNumber::I64(number)))
            } else if let Ok(number) = value.parse::<f64>() {
                Some(Val::Number(ValNumber::F64(number)))
            } else {
                return Err(AccessSolverError::Generic("Invalid number literal".into()));
            }
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
            missing_value_policy: MissingValuePolicy::Evaluate,
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

    /// A predicate that is either a boolean or a residue (such as a database predicate)
    #[derive(Debug, Clone, PartialEq)]
    enum TestPredicate {
        Bool(bool),
        Residue(String),
    }

    impl From<bool> for TestPredicate {
        fn from(value: bool) -> Self {
            TestPredicate::Bool(value)
        }
    }

    impl std::ops::Not for TestPredicate {
        type Output = Self;

        fn not(self) -> Self {
            match self {
                TestPredicate::Bool(value) => TestPredicate::Bool(!value),
                TestPredicate::Residue(residue) => TestPredicate::Residue(format!("!{residue}")),
            }
        }
    }

    impl AccessPredicate for TestPredicate {
        fn and(self, other: Self) -> Self {
            match (self, other) {
                (TestPredicate::Bool(false), _) | (_, TestPredicate::Bool(false)) => false.into(),
                (TestPredicate::Bool(true), other) | (other, TestPredicate::Bool(true)) => other,
                (TestPredicate::Residue(left), TestPredicate::Residue(right)) => {
                    TestPredicate::Residue(format!("({left} && {right})"))
                }
            }
        }

        fn or(self, other: Self) -> Self {
            match (self, other) {
                (TestPredicate::Bool(true), _) | (_, TestPredicate::Bool(true)) => true.into(),
                (TestPredicate::Bool(false), other) | (other, TestPredicate::Bool(false)) => other,
                (TestPredicate::Residue(left), TestPredicate::Residue(right)) => {
                    TestPredicate::Residue(format!("({left} || {right})"))
                }
            }
        }

        fn is_true(&self) -> bool {
            *self == TestPredicate::Bool(true)
        }

        fn is_false(&self) -> bool {
            *self == TestPredicate::Bool(false)
        }
    }

    #[test]
    fn test_combining_solved_and_unsolvable() {
        let solved = |value: bool| AccessSolution::Solved(TestPredicate::from(value));
        let unknown = AccessSolution::<TestPredicate>::unknown;
        let skipped = AccessSolution::<TestPredicate>::skipped;
        let published = || AccessSolution::Solved(TestPredicate::Residue("published".to_string()));
        let featured = || AccessSolution::Solved(TestPredicate::Residue("featured".to_string()));
        let residue = |residue: &str| TestPredicate::Residue(residue.to_string());

        let matrix = [
            // (expression, what it resolves to)
            // A solved side that decides the result
            (unknown().and(solved(false)), false.into()),
            (solved(false).and(unknown()).not(), true.into()),
            (unknown().or(solved(true)), true.into()),
            (solved(true).or(unknown()).not(), false.into()),
            // Otherwise, an unknown stays unknown, even under a negation
            (unknown(), false.into()),
            (unknown().not(), false.into()),
            (unknown().and(solved(true)).not(), false.into()),
            (solved(false).or(unknown()).not(), false.into()),
            // With a residue, true where the residue decides the result
            (unknown().or(published()), residue("published")),
            (unknown().or(published()).not(), false.into()),
            (unknown().and(published()), false.into()),
            (unknown().and(published()).not(), residue("!published")),
            (
                unknown().or(published()).and(featured()).not(),
                residue("!featured"),
            ),
            // A skipped comparison doesn't deny, even under a negation
            (skipped(), true.into()),
            (skipped().not(), true.into()),
            (skipped().and(solved(true)).not(), true.into()),
            (skipped().and(published()), residue("published")),
            (skipped().and(solved(false)), false.into()),
            // Solved solutions are combined (and negated) as before
            (solved(true).and(solved(false)).not(), true.into()),
            (solved(false).or(published()), residue("published")),
        ];

        for (index, (actual, expected)) in matrix.into_iter().enumerate() {
            assert_eq!(actual.resolve(), expected, "matrix entry {index}");
        }
    }
}
