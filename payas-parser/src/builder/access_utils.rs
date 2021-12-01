use payas_model::model::{
    access::{
        AccessConextSelection, AccessLogicalOp, AccessPredicateExpression,
        AccessPrimitiveExpression, AccessRelationalOp,
    },
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
    Context(AccessConextSelection, &'a GqlFieldType),
}

pub fn compute_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&GqlCompositeType>,
    building: &SystemContextBuilding,
) -> AccessPredicateExpression {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_selection(selection, self_type_info, building) {
                PathSelection::Column(column_id, column_type) => {
                    if column_type.base_type(&building.types.values).name == "Boolean" {
                        AccessPredicateExpression::BooleanColumn(column_id)
                    } else {
                        panic!("Field selection must be a boolean") // TODO: Result::Err
                    }
                }
                PathSelection::Context(context_selection, field_type) => {
                    if field_type.base_type(&building.types.values).name == "Boolean" {
                        AccessPredicateExpression::BooleanContextSelection(context_selection)
                    } else {
                        panic!("Context selection must be a boolean") // TODO: Result::Err
                    }
                }
            }
        }
        AstExpr::LogicalOp(op) => {
            let predicate_expr = |expr: &AstExpr<Typed>| {
                Box::new(compute_predicate_expression(expr, self_type_info, building))
            };
            match op {
                LogicalOp::And(left, right, _, _) => AccessPredicateExpression::LogicalOp(
                    AccessLogicalOp::And(predicate_expr(left), predicate_expr(right)),
                ),
                LogicalOp::Or(left, right, _, _) => AccessPredicateExpression::LogicalOp(
                    AccessLogicalOp::Or(predicate_expr(left), predicate_expr(right)),
                ),
                LogicalOp::Not(value, _, _) => AccessPredicateExpression::LogicalOp(
                    AccessLogicalOp::Not(predicate_expr(value)),
                ),
            }
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

            AccessPredicateExpression::RelationalOp(combiner(
                Box::new(compute_primitive_expr(left, self_type_info, building)),
                Box::new(compute_primitive_expr(right, self_type_info, building)),
            ))
        }
        AstExpr::BooleanLiteral(value, _) => AccessPredicateExpression::BooleanLiteral(*value),

        _ => panic!("Unsupported expression type"), // String or NumberLiteral cannot be used as a top-level expression in access rules
    }
}

fn compute_primitive_expr(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&GqlCompositeType>,
    building: &SystemContextBuilding,
) -> AccessPrimitiveExpression {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_selection(selection, self_type_info, building) {
                PathSelection::Column(column_id, _) => AccessPrimitiveExpression::Column(column_id),
                PathSelection::Context(c, _) => AccessPrimitiveExpression::ContextSelection(c),
            }
        }
        AstExpr::StringLiteral(value, _) => AccessPrimitiveExpression::StringLiteral(value.clone()),
        AstExpr::BooleanLiteral(value, _) => AccessPrimitiveExpression::BooleanLiteral(*value),
        AstExpr::NumberLiteral(value, _) => AccessPrimitiveExpression::NumberLiteral(*value),
        AstExpr::LogicalOp(_) => panic!(),
        AstExpr::RelationalOp(_) => panic!(),
    }
}

fn compute_selection<'a>(
    selection: &FieldSelection<Typed>,
    self_type_info: Option<&'a GqlCompositeType>,
    building: &'a SystemContextBuilding,
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

    fn get_context<'a>(
        path_elements: &[String],
        building: &'a SystemContextBuilding,
    ) -> (AccessConextSelection, &'a GqlFieldType) {
        if path_elements.len() == 2 {
            let context_type = building
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
                AccessConextSelection::Select(
                    Box::new(AccessConextSelection::Single(path_elements[0].clone())),
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

    if path_elements[0] == "self" {
        let (column_id, column_type) = get_column(&path_elements[1..], self_type_info.unwrap());
        PathSelection::Column(column_id, column_type)
    } else {
        let (context_selection, column_type) = get_context(&path_elements, building);
        PathSelection::Context(context_selection, column_type)
    }
}
