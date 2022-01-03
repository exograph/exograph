use payas_model::model::{
    access::{
        AccessContextSelection, AccessLogicalExpression, AccessPredicateExpression,
        AccessPrimitiveExpression, AccessRelationalOp,
    },
    column_id::ColumnId,
    relation::GqlRelation,
    GqlCompositeType, GqlFieldType, GqlTypeKind,
};

use crate::{
    ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp},
    error::ParserError,
    typechecker::Typed,
};

use super::system_builder::SystemContextBuilding;

enum PathSelection<'a> {
    Column(ColumnId, &'a GqlFieldType),
    Context(AccessContextSelection, &'a GqlFieldType),
}

pub fn compute_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&GqlCompositeType>,
    building: &SystemContextBuilding,
) -> Result<AccessPredicateExpression, ParserError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_selection(selection, self_type_info, building) {
                PathSelection::Column(column_id, column_type) => {
                    if column_type.base_type(&building.types.values).name == "Boolean" {
                        Ok(AccessPredicateExpression::BooleanColumn(column_id))
                    } else {
                        Err(ParserError::Generic(
                            "Field selection must be a boolean".to_string(),
                        ))
                    }
                }
                PathSelection::Context(context_selection, field_type) => {
                    if field_type.base_type(&building.types.values).name == "Boolean" {
                        Ok(AccessPredicateExpression::BooleanContextSelection(
                            context_selection,
                        ))
                    } else {
                        Err(ParserError::Generic(
                            "Context selection must be a boolean".to_string(),
                        ))
                    }
                }
            }
        }
        AstExpr::LogicalOp(op) => {
            let predicate_expr = |expr: &AstExpr<Typed>| {
                compute_predicate_expression(expr, self_type_info, building)
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
                Box::new(compute_primitive_expr(left, self_type_info, building)),
                Box::new(compute_primitive_expr(right, self_type_info, building)),
            )))
        }
        AstExpr::BooleanLiteral(value, _) => Ok(AccessPredicateExpression::BooleanLiteral(*value)),

        _ => Err(ParserError::Generic(
            "Unsupported expression type".to_string(),
        )), // String or NumberLiteral cannot be used as a top-level expression in access rules
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
        AstExpr::LogicalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
        AstExpr::RelationalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
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
        building: &'a SystemContextBuilding,
    ) -> (ColumnId, &'a GqlFieldType) {
        let get_field = |field_name: &str| {
            self_type_info
                .fields
                .iter()
                .find(|f| f.name == field_name)
                .unwrap_or_else(|| {
                    panic!(
                        "Field {} not found while processing access rules",
                        field_name
                    )
                })
        };

        match path_elements {
            [] => panic!("Cannot get column from an empty path"),
            [path_element] => {
                let field = get_field(path_element);
                match &field.relation {
                    GqlRelation::Pk { column_id }
                    | GqlRelation::Scalar { column_id }
                    | GqlRelation::ManyToOne { column_id, .. } => (column_id.clone(), &field.typ),
                    GqlRelation::OneToMany { .. } => todo!(),
                    GqlRelation::NonPersistent => panic!(),
                }
            }
            [head_element, rest @ ..] => {
                let field = get_field(head_element);

                let field_base_type = field.typ.base_type(&building.types.values);

                match &field_base_type.kind {
                    GqlTypeKind::Primitive => todo!(),
                    GqlTypeKind::Composite(self_type_info) => {
                        get_column(rest, self_type_info, building)
                    }
                }
            }
        }
    }

    fn get_context<'a>(
        path_elements: &[String],
        building: &'a SystemContextBuilding,
    ) -> (AccessContextSelection, &'a GqlFieldType) {
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
                AccessContextSelection::Select(
                    Box::new(AccessContextSelection::Single(path_elements[0].clone())),
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
        let (column_id, column_type) =
            get_column(&path_elements[1..], self_type_info.unwrap(), building);
        PathSelection::Column(column_id, column_type)
    } else {
        let (context_selection, column_type) = get_context(&path_elements, building);
        PathSelection::Context(context_selection, column_type)
    }
}
