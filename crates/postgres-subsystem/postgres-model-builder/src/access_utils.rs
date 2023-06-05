// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::{
    access::{AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp},
    context_type::{get_context, ContextFieldType, ContextSelection},
    mapped_arena::MappedArena,
    primitive_type::PrimitiveType,
    types::FieldType,
};
use core_plugin_interface::core_model_builder::{
    ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp},
    error::ModelBuildingError,
    typechecker::Typed,
};

use exo_sql::{Database, PhysicalColumnPath, PhysicalColumnPathLink};
use postgres_model::{
    access::DatabaseAccessPrimitiveExpression,
    types::{base_type, EntityType, PostgresFieldType, PostgresPrimitiveType, PostgresType},
};

use super::type_builder::ResolvedTypeEnv;

enum PathSelection<'a> {
    Column(
        PhysicalColumnPath,
        &'a FieldType<PostgresFieldType<EntityType>>,
    ),
    Context(ContextSelection, &'a ContextFieldType),
}

pub fn compute_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
    database: &Database,
) -> Result<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_selection(
                selection,
                self_type_info,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
                database,
            ) {
                PathSelection::Column(column_path, column_type) => {
                    if base_type(
                        column_type,
                        subsystem_primitive_types.values_ref(),
                        subsystem_entity_types.values_ref(),
                    )
                    .name()
                        == "Boolean"
                    {
                        // Treat boolean columns in the same way as an "eq" relational expression
                        // For example, treat `self.published` the same as `self.published == true`
                        Ok(AccessPredicateExpression::RelationalOp(
                            AccessRelationalOp::Eq(
                                Box::new(DatabaseAccessPrimitiveExpression::Column(column_path)),
                                Box::new(DatabaseAccessPrimitiveExpression::BooleanLiteral(true)),
                            ),
                        ))
                    } else {
                        Err(ModelBuildingError::Generic(
                            "Field selection must be a boolean".to_string(),
                        ))
                    }
                }
                PathSelection::Context(context_selection, field_type) => {
                    if field_type.innermost() == &PrimitiveType::Boolean {
                        // Treat boolean context expressions in the same way as an "eq" relational expression
                        // For example, treat `AuthContext.superUser` the same way as `AuthContext.superUser == true`
                        Ok(AccessPredicateExpression::RelationalOp(
                            AccessRelationalOp::Eq(
                                Box::new(DatabaseAccessPrimitiveExpression::ContextSelection(
                                    context_selection,
                                )),
                                Box::new(DatabaseAccessPrimitiveExpression::BooleanLiteral(true)),
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
        AstExpr::LogicalOp(op) => {
            let predicate_expr = |expr: &AstExpr<Typed>| {
                compute_predicate_expression(
                    expr,
                    self_type_info,
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
                    database,
                )
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
                Box::new(compute_primitive_expr(
                    left,
                    self_type_info,
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
                    database,
                )),
                Box::new(compute_primitive_expr(
                    right,
                    self_type_info,
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
                    database,
                )),
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
    self_type_info: Option<&EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
    database: &Database,
) -> DatabaseAccessPrimitiveExpression {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_selection(
                selection,
                self_type_info,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
                database,
            ) {
                PathSelection::Column(column_path, _) => {
                    DatabaseAccessPrimitiveExpression::Column(column_path)
                }
                PathSelection::Context(c, _) => {
                    DatabaseAccessPrimitiveExpression::ContextSelection(c)
                }
            }
        }
        AstExpr::StringLiteral(value, _) => {
            DatabaseAccessPrimitiveExpression::StringLiteral(value.clone())
        }
        AstExpr::BooleanLiteral(value, _) => {
            DatabaseAccessPrimitiveExpression::BooleanLiteral(*value)
        }
        AstExpr::NumberLiteral(value, _) => {
            DatabaseAccessPrimitiveExpression::NumberLiteral(*value)
        }
        AstExpr::StringList(_, _) => panic!("Access expressions do not support lists yet"),
        AstExpr::LogicalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
        AstExpr::RelationalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
    }
}

fn compute_selection<'a>(
    selection: &FieldSelection<Typed>,
    self_type_info: Option<&'a EntityType>,
    resolved_env: &'a ResolvedTypeEnv<'a>,
    subsystem_primitive_types: &'a MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &'a MappedArena<EntityType>,
    database: &Database,
) -> PathSelection<'a> {
    fn get_column<'a>(
        field_name: &str,
        self_type_info: &'a EntityType,
        database: &Database,
    ) -> (
        PhysicalColumnPathLink,
        &'a FieldType<PostgresFieldType<EntityType>>,
    ) {
        let get_field = |field_name: &str| {
            self_type_info.field_by_name(field_name).unwrap_or_else(|| {
                panic!("Field {field_name} not found while processing access rules")
            })
        };

        let field = get_field(field_name);
        let column_path_link = field.relation.column_path_link(database);

        (column_path_link, &field.typ)
    }

    let path_elements = selection.path();

    if path_elements[0] == "self" {
        let (_, column_path_elems, field_type) = path_elements[1..].iter().fold(
            (self_type_info, vec![], None),
            |(self_type_info, column_path_elems, _field_type), field_name| {
                let self_type_info =
                    self_type_info.expect("Type for the access selection is not defined");

                let (field_column_path, field_type) =
                    get_column(field_name, self_type_info, database);

                let field_composite_type = match base_type(
                    field_type,
                    subsystem_primitive_types.values_ref(),
                    subsystem_entity_types.values_ref(),
                ) {
                    PostgresType::Composite(composite_type) => Some(composite_type),
                    _ => None,
                };

                (
                    field_composite_type,
                    column_path_elems
                        .into_iter()
                        .chain(vec![field_column_path])
                        .collect(),
                    Some(field_type),
                )
            },
        );

        PathSelection::Column(
            PhysicalColumnPath {
                path: column_path_elems,
            },
            field_type.unwrap(),
        )
    } else {
        let (context_selection, context_field_type) =
            get_context(&path_elements, resolved_env.contexts);
        PathSelection::Context(context_selection, context_field_type)
    }
}
