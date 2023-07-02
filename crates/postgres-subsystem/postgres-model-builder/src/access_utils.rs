// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::{
    access::{
        AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
        CommonAccessPrimitiveExpression,
    },
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

use exo_sql::{ColumnPathLink, Database, PhysicalColumnPath};
use postgres_model::{
    access::{DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression},
    types::{
        base_type, EntityType, PostgresField, PostgresFieldType, PostgresPrimitiveType,
        PostgresType,
    },
};

use super::type_builder::ResolvedTypeEnv;

enum DatabasePathSelection<'a> {
    Column(
        PhysicalColumnPath,
        &'a FieldType<PostgresFieldType<EntityType>>,
    ),
    Context(ContextSelection, &'a ContextFieldType),
}

enum JsonPathSelection<'a> {
    Path(Vec<String>, &'a FieldType<PostgresFieldType<EntityType>>),
    Context(ContextSelection, &'a ContextFieldType),
}

pub fn compute_input_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
) -> Result<AccessPredicateExpression<InputAccessPrimitiveExpression>, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_json_selection(
                selection,
                self_type_info,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
            ) {
                JsonPathSelection::Path(_, _) => Err(ModelBuildingError::Generic(
                    "Top-level path selection of just `self` not allowed".to_string(),
                )),
                JsonPathSelection::Context(context_selection, field_type) => {
                    if field_type.innermost() == &PrimitiveType::Boolean {
                        // Treat boolean context expressions in the same way as an "eq" relational expression
                        // For example, treat `AuthContext.superUser` the same way as `AuthContext.superUser == true`
                        Ok(AccessPredicateExpression::RelationalOp(
                            AccessRelationalOp::Eq(
                                Box::new(InputAccessPrimitiveExpression::Common(
                                    CommonAccessPrimitiveExpression::ContextSelection(
                                        context_selection,
                                    ),
                                )),
                                Box::new(InputAccessPrimitiveExpression::Common(
                                    CommonAccessPrimitiveExpression::BooleanLiteral(true),
                                )),
                            ),
                        ))
                    } else {
                        Err(ModelBuildingError::Generic(
                            "Top-level context selection must be a boolean".to_string(),
                        ))
                    }
                }
            }
        }
        AstExpr::LogicalOp(op) => {
            let predicate_expr = |expr: &AstExpr<Typed>| {
                compute_input_predicate_expression(
                    expr,
                    self_type_info,
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
                )
            };
            compute_logical_op(op, predicate_expr)
        }
        AstExpr::RelationalOp(op) => {
            let primitive_expr = |expr: &AstExpr<Typed>| {
                compute_primitive_json_expr(
                    expr,
                    self_type_info,
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
                )
            };
            compute_relatinal_op(op, primitive_expr)
        }
        AstExpr::BooleanLiteral(value, _) => Ok(AccessPredicateExpression::BooleanLiteral(*value)),
        AstExpr::StringLiteral(_, _) => Err(ModelBuildingError::Generic(
            "Top-level expression cannot be a string literal".to_string(),
        )),
        AstExpr::NumberLiteral(_, _) => Err(ModelBuildingError::Generic(
            "Top-level expression cannot be a number literal".to_string(),
        )),
        AstExpr::StringList(_, _) => Err(ModelBuildingError::Generic(
            "Top-level expression cannot be a list literal".to_string(),
        )),
    }
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
            match compute_column_selection(
                selection,
                self_type_info,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
                database,
            ) {
                DatabasePathSelection::Column(column_path, column_type) => {
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
                                Box::new(DatabaseAccessPrimitiveExpression::Common(
                                    CommonAccessPrimitiveExpression::BooleanLiteral(true),
                                )),
                            ),
                        ))
                    } else {
                        Err(ModelBuildingError::Generic(
                            "Field selection must be a boolean".to_string(),
                        ))
                    }
                }
                DatabasePathSelection::Context(context_selection, field_type) => {
                    if field_type.innermost() == &PrimitiveType::Boolean {
                        // Treat boolean context expressions in the same way as an "eq" relational expression
                        // For example, treat `AuthContext.superUser` the same way as `AuthContext.superUser == true`
                        Ok(AccessPredicateExpression::RelationalOp(
                            AccessRelationalOp::Eq(
                                Box::new(DatabaseAccessPrimitiveExpression::Common(
                                    CommonAccessPrimitiveExpression::ContextSelection(
                                        context_selection,
                                    ),
                                )),
                                Box::new(DatabaseAccessPrimitiveExpression::Common(
                                    CommonAccessPrimitiveExpression::BooleanLiteral(true),
                                )),
                            ),
                        ))
                    } else {
                        Err(ModelBuildingError::Generic(
                            "Top-level context selection must be a boolean".to_string(),
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
            compute_logical_op(op, predicate_expr)
        }
        AstExpr::RelationalOp(op) => {
            let predicate_expr = |expr: &AstExpr<Typed>| {
                compute_primitive_db_expr(
                    expr,
                    self_type_info,
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
                    database,
                )
            };
            compute_relatinal_op(op, predicate_expr)
        }
        AstExpr::BooleanLiteral(value, _) => Ok(AccessPredicateExpression::BooleanLiteral(*value)),

        _ => Err(ModelBuildingError::Generic(
            "Unsupported expression type".to_string(),
        )), // String or NumberLiteral cannot be used as a top-level expression in access rules
    }
}

fn compute_primitive_db_expr(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
    database: &Database,
) -> DatabaseAccessPrimitiveExpression {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_column_selection(
                selection,
                self_type_info,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
                database,
            ) {
                DatabasePathSelection::Column(column_path, _) => {
                    DatabaseAccessPrimitiveExpression::Column(column_path)
                }
                DatabasePathSelection::Context(c, _) => DatabaseAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::ContextSelection(c),
                ),
            }
        }
        AstExpr::StringLiteral(value, _) => DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::StringLiteral(value.clone()),
        ),
        AstExpr::BooleanLiteral(value, _) => DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::BooleanLiteral(*value),
        ),
        AstExpr::NumberLiteral(value, _) => DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NumberLiteral(*value),
        ),
        AstExpr::StringList(_, _) => panic!("Access expressions do not support lists yet"),
        AstExpr::LogicalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
        AstExpr::RelationalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
    }
}

fn compute_primitive_json_expr(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
) -> InputAccessPrimitiveExpression {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_json_selection(
                selection,
                self_type_info,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
            ) {
                JsonPathSelection::Path(path, _) => InputAccessPrimitiveExpression::Path(path),
                JsonPathSelection::Context(c, _) => InputAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::ContextSelection(c),
                ),
            }
        }
        AstExpr::StringLiteral(value, _) => InputAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::StringLiteral(value.clone()),
        ),
        AstExpr::BooleanLiteral(value, _) => InputAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::BooleanLiteral(*value),
        ),
        AstExpr::NumberLiteral(value, _) => InputAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NumberLiteral(*value),
        ),
        AstExpr::StringList(_, _) => panic!("Access expressions do not support lists yet"),
        AstExpr::LogicalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
        AstExpr::RelationalOp(_) => unreachable!(), // Parser has already ensures that the two sides are primitive expressions
    }
}

fn compute_logical_op<PrimExpr: Send + Sync>(
    op: &LogicalOp<Typed>,
    predicate_expr: impl Fn(
        &AstExpr<Typed>,
    ) -> Result<AccessPredicateExpression<PrimExpr>, ModelBuildingError>,
) -> Result<AccessPredicateExpression<PrimExpr>, ModelBuildingError> {
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

fn compute_relatinal_op<PrimExpr: Send + Sync>(
    op: &RelationalOp<Typed>,
    primitive_expr: impl Fn(&AstExpr<Typed>) -> PrimExpr,
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

    Ok(AccessPredicateExpression::RelationalOp(combiner(
        Box::new(primitive_expr(left)),
        Box::new(primitive_expr(right)),
    )))
}

fn compute_column_selection<'a>(
    selection: &FieldSelection<Typed>,
    self_type_info: Option<&'a EntityType>,
    resolved_env: &'a ResolvedTypeEnv<'a>,
    subsystem_primitive_types: &'a MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &'a MappedArena<EntityType>,
    database: &Database,
) -> DatabasePathSelection<'a> {
    fn get_column<'a>(
        field_name: &str,
        self_type_info: &'a EntityType,
        database: &Database,
    ) -> (ColumnPathLink, &'a FieldType<PostgresFieldType<EntityType>>) {
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
        let (_, column_path, field_type) = path_elements[1..].iter().fold(
            (self_type_info, None::<PhysicalColumnPath>, None),
            |(self_type_info, column_path, _field_type), field_name| {
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

                let new_column_path = match column_path {
                    Some(column_path) => Some(column_path.push(field_column_path)),
                    None => Some(PhysicalColumnPath::init(field_column_path)),
                };
                (field_composite_type, new_column_path, Some(field_type))
            },
        );

        // TODO: Avoid this unwrap (parser should have caught expression "self" without any fields)
        DatabasePathSelection::Column(column_path.unwrap(), field_type.unwrap())
    } else {
        let (context_selection, context_field_type) =
            get_context(&path_elements, resolved_env.contexts);
        DatabasePathSelection::Context(context_selection, context_field_type)
    }
}

fn compute_json_selection<'a>(
    selection: &FieldSelection<Typed>,
    self_type_info: Option<&'a EntityType>,
    resolved_env: &'a ResolvedTypeEnv<'a>,
    subsystem_primitive_types: &'a MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &'a MappedArena<EntityType>,
) -> JsonPathSelection<'a> {
    fn get_field<'a>(
        field_name: &str,
        self_type_info: &'a EntityType,
    ) -> &'a PostgresField<EntityType> {
        self_type_info
            .field_by_name(field_name)
            .unwrap_or_else(|| panic!("Field {field_name} not found while processing access rules"))
    }

    let path_elements = selection.path();

    if path_elements[0] == "self" {
        let (_, json_path, field_type) = path_elements[1..].iter().fold(
            (self_type_info, Vec::new(), None),
            |(self_type_info, json_path, _field_type), field_name| {
                let self_type_info =
                    self_type_info.expect("Type for the access selection is not defined");

                let field = get_field(field_name, self_type_info);
                let field_type = &field.typ;

                let field_composite_type = match base_type(
                    field_type,
                    subsystem_primitive_types.values_ref(),
                    subsystem_entity_types.values_ref(),
                ) {
                    PostgresType::Composite(composite_type) => Some(composite_type),
                    _ => None,
                };

                let field_name = field_name.clone();
                let mut json_path = json_path;
                json_path.push(field_name);

                (field_composite_type, json_path, Some(field_type))
            },
        );

        // TODO: Avoid this unwrap (parser should have caught expression "self" without any fields)
        JsonPathSelection::Path(json_path, field_type.unwrap())
    } else {
        let (context_selection, context_field_type) =
            get_context(&path_elements, resolved_env.contexts);
        JsonPathSelection::Context(context_selection, context_field_type)
    }
}
