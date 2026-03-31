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
        CommonAccessPrimitiveExpression, FunctionCall,
    },
    mapped_arena::SerializableSlabIndex,
};
use core_resolver::access_solver::AccessSolverError;

use exo_sql_pg::{ColumnPathLink, PhysicalColumnPath};
use serde::{Deserialize, Serialize};

use crate::types::{EntityType, PostgresFieldDefaultValue};

/// Access specification for a model
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Access {
    pub creation: CreationAccessExpression,
    pub read: SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    pub update: UpdateAccessExpression,
    pub delete: SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreationAccessExpression {
    pub precheck:
        SerializableSlabIndex<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateAccessExpression {
    pub precheck:
        SerializableSlabIndex<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>>,
    pub database:
        SerializableSlabIndex<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
}

/// Primitive expression (that doesn't contain any other expressions).
/// Used as sides of `AccessRelationalExpression` to form more complex expressions
/// such as equal and less than.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DatabaseAccessPrimitiveExpression {
    Column(PhysicalColumnPath, Option<String>), // Column path, for example self.user.id and parameter name (such as "du", default: "self")
    Function(PhysicalColumnPath, FunctionCall<Self>), // Function, for example self.documentUser.some(du => du.id == AuthContext.id && du.read)
    Common(CommonAccessPrimitiveExpression),          // expression shared by all access expressions
}

/// Primitive expressions that can express data input access control rules.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PrecheckAccessPrimitiveExpression {
    Path(AccessPrimitiveExpressionPath, Option<String>), // JSON path, for example self.user.id and parameter name (such as "du", default: "self")
    Function(AccessPrimitiveExpressionPath, FunctionCall<Self>), // Function, for example self.documentUser.some(du => du.id == AuthContext.id && du.read)
    Common(CommonAccessPrimitiveExpression), // expression shared by all access expressions
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AccessPrimitiveExpressionPath {
    pub column_path: PhysicalColumnPath,
    pub field_path: FieldPath,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FieldPath {
    Normal(Vec<String>, Option<PostgresFieldDefaultValue>), // Non-pk field path such as self.title
    Pk {
        // pk field path such as self.project.owner.id
        lead: Vec<String>,                               // project
        lead_default: Option<PostgresFieldDefaultValue>, // default value for the lead field
        pk_fields: Vec<String>,                          // id (pk fields of Project)
    },
}

impl AccessPrimitiveExpressionPath {
    pub fn new(column_path: PhysicalColumnPath, field_path: FieldPath) -> Self {
        Self {
            column_path,
            field_path,
        }
    }

    pub fn with_function_context(
        self,
        other: Self,
        parameter_name: String,
    ) -> Result<Self, AccessSolverError> {
        Ok(Self {
            column_path: self.column_path.join(other.column_path),
            field_path: match other.field_path {
                FieldPath::Normal(b, default) => {
                    let mut new_field_path = vec![parameter_name.to_string()];
                    new_field_path.extend(b);
                    FieldPath::Normal(new_field_path, default)
                }
                FieldPath::Pk {
                    lead,
                    lead_default,
                    pk_fields,
                } => {
                    let mut new_field_path = vec![parameter_name.to_string()];
                    new_field_path.extend(lead);
                    FieldPath::Pk {
                        lead: new_field_path,
                        lead_default,
                        pk_fields: pk_fields.clone(),
                    }
                }
            },
        })
    }
}

/// Extract the parent-scoped portion of a child entity's access predicate.
///
/// Given a child entity's database access predicate (e.g., `Document.self.user.id =
/// AuthContext.id`) and a parent entity (e.g., `User`), this function computes a predicate
/// that can be applied to the parent entity (e.g., `User.self.id = AuthContext.id`).
///
/// This is used to restrict which parent rows get nested inserts applied to them,
/// ensuring that access control rules on the child entity are respected.
pub fn parent_predicate(
    expr: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    parent_entity: &EntityType,
) -> Result<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>, String> {
    let reduced = reduce_nested_predicate(expr, parent_entity)?;

    Ok(match reduced {
        NestedPredicatePart::Parent(expr) => expr,
        _ => AccessPredicateExpression::BooleanLiteral(true),
    })
}

enum NestedPredicatePart<T> {
    // Uses only the parent elements
    Parent(T),
    // Uses only the nested elements
    Nested(T),
    // Constants, context selection etc
    Common(T),
}

fn reduce_nested_predicate(
    expr: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    parent_entity: &EntityType,
) -> Result<NestedPredicatePart<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>, String>
{
    match expr {
        AccessPredicateExpression::LogicalOp(op) => reduce_nested_logical_op(op, parent_entity),
        AccessPredicateExpression::RelationalOp(op) => {
            reduce_nested_relational_op(op, parent_entity)
        }
        AccessPredicateExpression::BooleanLiteral(_) => Ok(NestedPredicatePart::Common(expr)),
    }
}

fn reduce_nested_logical_op(
    op: AccessLogicalExpression<DatabaseAccessPrimitiveExpression>,
    parent_entity: &EntityType,
) -> Result<NestedPredicatePart<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>, String>
{
    fn combine(
        lhs: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
        rhs: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
        parent_entity: &EntityType,
        combiner: impl Fn(
            Box<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
            Box<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
        ) -> AccessLogicalExpression<DatabaseAccessPrimitiveExpression>,
    ) -> Result<
        NestedPredicatePart<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
        String,
    > {
        let lhs = reduce_nested_predicate(lhs, parent_entity)?;
        let rhs = reduce_nested_predicate(rhs, parent_entity)?;

        match (lhs, rhs) {
            (NestedPredicatePart::Parent(lhs), NestedPredicatePart::Parent(rhs))
            | (NestedPredicatePart::Common(lhs), NestedPredicatePart::Parent(rhs))
            | (NestedPredicatePart::Parent(lhs), NestedPredicatePart::Common(rhs)) => {
                Ok(NestedPredicatePart::Parent(
                    AccessPredicateExpression::LogicalOp(combiner(Box::new(lhs), Box::new(rhs))),
                ))
            }
            (NestedPredicatePart::Parent(p), NestedPredicatePart::Nested(_))
            | (NestedPredicatePart::Nested(_), NestedPredicatePart::Parent(p)) => {
                // If one side of and/or is a nested expression, then we can eliminate it by returns just the parent expression
                Ok(NestedPredicatePart::Parent(p))
            }
            (NestedPredicatePart::Nested(_), NestedPredicatePart::Nested(_))
            | (NestedPredicatePart::Common(_), NestedPredicatePart::Nested(_))
            | (NestedPredicatePart::Nested(_), NestedPredicatePart::Common(_))
            | (NestedPredicatePart::Common(_), NestedPredicatePart::Common(_)) => Ok(
                NestedPredicatePart::Common(AccessPredicateExpression::BooleanLiteral(true)),
            ),
        }
    }

    match op {
        AccessLogicalExpression::Not(e) => {
            let e = reduce_nested_predicate(*e, parent_entity)?;

            Ok(match e {
                NestedPredicatePart::Parent(e) => NestedPredicatePart::Parent(
                    AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Not(Box::new(e))),
                ),
                _ => {
                    // If the underlying expression does not use the parent entity, then we eliminate it by replacing it with a constant true
                    NestedPredicatePart::Common(AccessPredicateExpression::BooleanLiteral(true))
                }
            })
        }
        AccessLogicalExpression::And(lhs, rhs) => {
            combine(*lhs, *rhs, parent_entity, AccessLogicalExpression::And)
        }
        AccessLogicalExpression::Or(lhs, rhs) => {
            combine(*lhs, *rhs, parent_entity, AccessLogicalExpression::Or)
        }
    }
}

fn reduce_nested_relational_op(
    op: AccessRelationalOp<DatabaseAccessPrimitiveExpression>,
    parent_entity: &EntityType,
) -> Result<NestedPredicatePart<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>, String>
{
    fn combine(
        lhs: DatabaseAccessPrimitiveExpression,
        rhs: DatabaseAccessPrimitiveExpression,
        parent_entity: &EntityType,
        combiner: impl Fn(
            Box<DatabaseAccessPrimitiveExpression>,
            Box<DatabaseAccessPrimitiveExpression>,
        ) -> AccessRelationalOp<DatabaseAccessPrimitiveExpression>,
    ) -> Result<
        NestedPredicatePart<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
        String,
    > {
        let reduced_lhs = reduce_nested_primitive_expr(lhs, parent_entity);
        let reduced_rhs = reduce_nested_primitive_expr(rhs, parent_entity);

        match (reduced_lhs, reduced_rhs) {
            (NestedPredicatePart::Parent(l), NestedPredicatePart::Parent(r))
            | (NestedPredicatePart::Common(l), NestedPredicatePart::Parent(r))
            | (NestedPredicatePart::Parent(l), NestedPredicatePart::Common(r)) => {
                Ok(NestedPredicatePart::Parent(
                    AccessPredicateExpression::RelationalOp(combiner(Box::new(l), Box::new(r))),
                ))
            }
            (NestedPredicatePart::Nested(l), NestedPredicatePart::Nested(r))
            | (NestedPredicatePart::Common(l), NestedPredicatePart::Nested(r))
            | (NestedPredicatePart::Nested(l), NestedPredicatePart::Common(r)) => {
                Ok(NestedPredicatePart::Nested(
                    AccessPredicateExpression::RelationalOp(combiner(Box::new(l), Box::new(r))),
                ))
            }
            (NestedPredicatePart::Common(l), NestedPredicatePart::Common(r)) => {
                Ok(NestedPredicatePart::Common(
                    AccessPredicateExpression::RelationalOp(combiner(Box::new(l), Box::new(r))),
                ))
            }
            (NestedPredicatePart::Parent(_), NestedPredicatePart::Nested(_))
            | (NestedPredicatePart::Nested(_), NestedPredicatePart::Parent(_)) => Err(
                "Access expression comparing a parent field with a nested field is not supported"
                    .to_string(),
            ),
        }
    }

    let combiner = op.combiner();
    let (l, r) = op.owned_sides();
    combine(*l, *r, parent_entity, combiner)
}

fn reduce_nested_primitive_expr(
    expr: DatabaseAccessPrimitiveExpression,
    parent_entity: &EntityType,
) -> NestedPredicatePart<DatabaseAccessPrimitiveExpression> {
    match expr {
        DatabaseAccessPrimitiveExpression::Column(ref pc, ref parameter_name) => {
            let (head, tail) = pc.split_head();

            match head {
                ColumnPathLink::Relation(r) if r.linked_table_id == parent_entity.table_id => {
                    // Eliminate the head link. For example if the expression is self.user.id, then
                    // we can reduce it to just id (assuming that the parent entity is user)
                    NestedPredicatePart::Parent(DatabaseAccessPrimitiveExpression::Column(
                        tail.unwrap(),
                        parameter_name.clone(),
                    ))
                }
                _ => NestedPredicatePart::Nested(expr),
            }
        }
        DatabaseAccessPrimitiveExpression::Function(ref pc, ref fc) => {
            let (head, tail) = pc.split_head();

            match head {
                ColumnPathLink::Relation(r) if r.linked_table_id == parent_entity.table_id => {
                    NestedPredicatePart::Parent(DatabaseAccessPrimitiveExpression::Column(
                        tail.unwrap(),
                        Some(fc.parameter_name.clone()),
                    ))
                }
                _ => NestedPredicatePart::Nested(expr),
            }
        }
        DatabaseAccessPrimitiveExpression::Common(_) => NestedPredicatePart::Common(expr),
    }
}
