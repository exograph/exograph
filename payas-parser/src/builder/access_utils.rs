use payas_model::model::{
    access::{AccessConextSelection, AccessExpression, AccessLogicalOp, AccessRelationalOp},
    column_id::ColumnId,
    relation::GqlRelation,
    GqlCompositeType, GqlFieldType,
};

use crate::{
    ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp},
    typechecker::Typed,
};

use super::system_builder::SystemContextBuilding;

enum PathSelection<'a> {
    Column(ColumnId, &'a GqlFieldType),
    Context(AccessConextSelection),
}

fn compute_selection<'a>(
    selection: &FieldSelection<Typed>,
    self_type_info: Option<&'a GqlCompositeType>,
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

    fn unflatten(elements: &[String]) -> AccessConextSelection {
        if elements.len() == 1 {
            AccessConextSelection::Single(elements[0].clone())
        } else {
            AccessConextSelection::Select(
                Box::new(unflatten(&elements[..elements.len() - 1])),
                elements.last().unwrap().clone(),
            )
        }
    }

    fn get_column<'a>(
        path_elements: &[String],
        self_type_info: &'a GqlCompositeType,
    ) -> (ColumnId, &'a GqlFieldType) {
        if path_elements.len() == 1 {
            let field = self_type_info
                .fields
                .iter()
                .find(|field| field.name == path_elements[0])
                .unwrap();
            match &field.relation {
                GqlRelation::Pk { column_id }
                | GqlRelation::Scalar { column_id }
                | GqlRelation::ManyToOne { column_id, .. } => (column_id.clone(), &field.typ),
                GqlRelation::OneToMany { .. } => todo!(),
                GqlRelation::NonPersistent => panic!(),
            }
        } else {
            todo!() // Nested selection such as self.venue.published
        }
    }

    let mut path_elements = vec![];
    flatten(selection, &mut path_elements);

    if path_elements[0] == "self" {
        let (column_id, column_type) = get_column(&path_elements[1..], self_type_info.unwrap());
        PathSelection::Column(column_id, column_type)
    } else {
        PathSelection::Context(unflatten(&path_elements))
    }
}

pub fn compute_expression(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&GqlCompositeType>,
    building: &SystemContextBuilding,
    coerce_boolean: bool,
) -> AccessExpression {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_selection(selection, self_type_info) {
                PathSelection::Column(column_id, column_type) => {
                    let column = AccessExpression::Column(column_id);

                    // Coerces the result into an equivalent RelationalOp if `coerce_boolean` is true
                    // For example, exapnds `self.published` to `self.published == true`, if `published` is a boolean column
                    // This allows specifying access rule such as `AuthContext.role == "ROLE_ADMIN" || self.published` instead of
                    // AuthContext.role == "ROLE_ADMIN" || self.published == true`
                    if coerce_boolean
                        && column_type.base_type(&building.types.values).name == "Boolean"
                    {
                        AccessExpression::RelationalOp(AccessRelationalOp::Eq(
                            Box::new(column),
                            Box::new(AccessExpression::BooleanLiteral(true)),
                        ))
                    } else {
                        column
                    }
                }
                PathSelection::Context(c) => AccessExpression::ContextSelection(c),
            }
        }
        AstExpr::LogicalOp(op) => match op {
            LogicalOp::And(left, right, _, _) => AccessExpression::LogicalOp(AccessLogicalOp::And(
                Box::new(compute_expression(left, self_type_info, building, true)),
                Box::new(compute_expression(right, self_type_info, building, true)),
            )),
            LogicalOp::Or(left, right, _, _) => AccessExpression::LogicalOp(AccessLogicalOp::Or(
                Box::new(compute_expression(left, self_type_info, building, true)),
                Box::new(compute_expression(right, self_type_info, building, true)),
            )),
            LogicalOp::Not(value, _, _) => AccessExpression::LogicalOp(AccessLogicalOp::Not(
                Box::new(compute_expression(value, self_type_info, building, true)),
            )),
        },
        AstExpr::RelationalOp(op) => match op {
            RelationalOp::Eq(left, right, _) => {
                AccessExpression::RelationalOp(AccessRelationalOp::Eq(
                    Box::new(compute_expression(left, self_type_info, building, false)),
                    Box::new(compute_expression(right, self_type_info, building, false)),
                ))
            }
            RelationalOp::Neq(_left, _right, _) => {
                todo!()
            }
            RelationalOp::Lt(_left, _right, _) => {
                todo!()
            }
            RelationalOp::Lte(_left, _right, _) => {
                todo!()
            }
            RelationalOp::Gt(_left, _right, _) => {
                todo!()
            }
            RelationalOp::Gte(_left, _right, _) => {
                todo!()
            }
            RelationalOp::In(left, right, _) => {
                AccessExpression::RelationalOp(AccessRelationalOp::In(
                    Box::new(compute_expression(left, self_type_info, building, false)),
                    Box::new(compute_expression(right, self_type_info, building, false)),
                ))
            }
        },
        AstExpr::StringLiteral(value, _) => AccessExpression::StringLiteral(value.clone()),
        AstExpr::BooleanLiteral(value, _) => AccessExpression::BooleanLiteral(*value),
        AstExpr::NumberLiteral(value, _) => AccessExpression::NumberLiteral(*value),
    }
}
