// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_plugin_interface::{
    core_model::access::FunctionCall,
    core_model_builder::{
        ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp},
        error::ModelBuildingError,
        typechecker::Typed,
    },
};
use core_plugin_interface::{
    core_model::{
        access::{
            AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
            CommonAccessPrimitiveExpression,
        },
        context_type::{ContextFieldType, ContextSelection},
        mapped_arena::MappedArena,
        primitive_type::PrimitiveType,
        types::FieldType,
    },
    core_model_builder::ast::ast_types::FieldSelectionElement,
};

use exo_sql::{ColumnPathLink, Database, PhysicalColumnPath};
use postgres_core_model::types::{
    base_type, EntityType, PostgresFieldType, PostgresPrimitiveType, PostgresType,
};
use postgres_core_model::{
    access::{DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression},
    types::PostgresField,
};

use crate::resolved_type::ResolvedTypeEnv;

enum DatabasePathSelection<'a> {
    Column(
        PhysicalColumnPath,
        &'a FieldType<PostgresFieldType<EntityType>>,
        Option<String>, // Parameter name (such as "du", default: "self")
    ),
    Function(
        PhysicalColumnPath,
        FunctionCall<DatabaseAccessPrimitiveExpression>,
    ), // Function, for example self.documentUser.some(du => du.id == AuthContext.id && du.read)
    Context(ContextSelection, &'a ContextFieldType),
}

enum JsonPathSelection<'a> {
    Path(
        Vec<String>,
        &'a FieldType<PostgresFieldType<EntityType>>,
        Option<String>, // Parameter name (such as "du", default: "self")
    ),
    Function(Vec<String>, FunctionCall<InputAccessPrimitiveExpression>), // Function, for example self.documentUser.some(du => du.id == AuthContext.id && du.read)
    Context(ContextSelection, &'a ContextFieldType),
}

pub fn compute_input_predicate_expression(
    expr: &AstExpr<Typed>,
    scope: HashMap<String, &EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
) -> Result<AccessPredicateExpression<InputAccessPrimitiveExpression>, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            let json_selection = compute_json_selection(
                selection,
                scope,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
            )?;

            match json_selection {
                JsonPathSelection::Path(path, field_type, parameter_name) => {
                    let field_entity_type = field_type.innermost().type_id.to_type(
                        subsystem_primitive_types.values_ref(),
                        subsystem_entity_types.values_ref(),
                    );

                    if field_entity_type.name() == "Boolean" {
                        // Treat boolean context expressions in the same way as an "eq" relational expression
                        // For example, treat `AuthContext.superUser` the same way as `AuthContext.superUser == true`
                        Ok(AccessPredicateExpression::RelationalOp(
                            AccessRelationalOp::Eq(
                                Box::new(InputAccessPrimitiveExpression::Path(
                                    path,
                                    parameter_name,
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
                JsonPathSelection::Function(lead_path, function_call) => {
                    compute_input_function_expr(
                        lead_path,
                        function_call.parameter_name,
                        function_call.expr,
                    )
                }
            }
        }
        AstExpr::LogicalOp(op) => {
            let predicate_expr = |expr: &AstExpr<Typed>| {
                compute_input_predicate_expression(
                    expr,
                    scope.clone(),
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
                    scope.clone(),
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
                )
            };
            compute_relational_op(op, primitive_expr)
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
        AstExpr::NullLiteral(_) => Err(ModelBuildingError::Generic(
            "Top-level expression cannot be a null literal".to_string(),
        )),
    }
}

pub fn compute_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: &EntityType,
    function_context: HashMap<String, &EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
    database: &Database,
) -> Result<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            let column_selection = compute_column_selection(
                selection,
                self_type_info,
                resolved_env,
                function_context,
                subsystem_primitive_types,
                subsystem_entity_types,
                database,
            )?;

            match column_selection {
                DatabasePathSelection::Column(column_path, column_type, parameter_name) => {
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
                                Box::new(DatabaseAccessPrimitiveExpression::Column(
                                    column_path,
                                    parameter_name,
                                )),
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
                DatabasePathSelection::Function(column_path, function_call) => {
                    if function_call.name != "some" {
                        Err(ModelBuildingError::Generic(
                            "Only `some` function is supported".to_string(),
                        ))
                    } else {
                        compute_function_expr(
                            column_path,
                            function_call.parameter_name,
                            function_call.expr,
                        )
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
                    function_context.clone(),
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
                    function_context.clone(),
                    subsystem_primitive_types,
                    subsystem_entity_types,
                    database,
                )
            };

            compute_relational_op(op, predicate_expr)
        }
        AstExpr::BooleanLiteral(value, _) => Ok(AccessPredicateExpression::BooleanLiteral(*value)),

        _ => Err(ModelBuildingError::Generic(
            "Unsupported expression type".to_string(),
        )), // String or NumberLiteral cannot be used as a top-level expression in access rules
    }
}

fn compute_function_expr(
    lead_path: PhysicalColumnPath,
    function_param_name: String,
    function_expr: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
) -> Result<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>, ModelBuildingError> {
    fn function_elem_path(
        lead_path: PhysicalColumnPath,
        function_param_name: String,
        expr: DatabaseAccessPrimitiveExpression,
    ) -> Result<DatabaseAccessPrimitiveExpression, ModelBuildingError> {
        match expr {
            DatabaseAccessPrimitiveExpression::Column(function_column_path, parameter_name) => {
                // We may have expression like `self.documentUser.some(du => du.read)`, in which case we want to join the column path
                // to form `self.documentUser.read`.
                //
                // However, if the lead path is `self.documentUser.some(du => du.id === self.id)`, we don't want to join the column path
                // for the `self.id` part.
                Ok(DatabaseAccessPrimitiveExpression::Column(
                    if parameter_name == Some(function_param_name) {
                        lead_path.clone().join(function_column_path)
                    } else {
                        function_column_path
                    },
                    parameter_name,
                ))
            }
            DatabaseAccessPrimitiveExpression::Function(_, _) => Err(ModelBuildingError::Generic(
                "Cannot have a function call inside another function call".to_string(),
            )),
            expr => Ok(expr),
        }
    }

    match function_expr {
        AccessPredicateExpression::LogicalOp(op) => match op {
            AccessLogicalExpression::Not(p) => {
                let updated_expr = compute_function_expr(lead_path, function_param_name, *p)?;
                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Not(Box::new(updated_expr)),
                ))
            }
            AccessLogicalExpression::And(left, right) => {
                let updated_left =
                    compute_function_expr(lead_path.clone(), function_param_name.clone(), *left)?;
                let updated_right = compute_function_expr(lead_path, function_param_name, *right)?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::And(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
            AccessLogicalExpression::Or(left, right) => {
                let updated_left =
                    compute_function_expr(lead_path.clone(), function_param_name.clone(), *left)?;
                let updated_right = compute_function_expr(lead_path, function_param_name, *right)?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Or(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
        },
        AccessPredicateExpression::RelationalOp(op) => {
            let combiner = op.combiner();
            let (left, right) = op.owned_sides();

            let updated_left =
                function_elem_path(lead_path.clone(), function_param_name.clone(), *left)?;
            let updated_right = function_elem_path(lead_path, function_param_name, *right)?;
            Ok(AccessPredicateExpression::RelationalOp(combiner(
                Box::new(updated_left),
                Box::new(updated_right),
            )))
        }
        AccessPredicateExpression::BooleanLiteral(value) => {
            Ok(AccessPredicateExpression::BooleanLiteral(value))
        }
    }
}

fn compute_input_function_expr(
    lead_path: Vec<String>,
    function_param_name: String,
    function_expr: AccessPredicateExpression<InputAccessPrimitiveExpression>,
) -> Result<AccessPredicateExpression<InputAccessPrimitiveExpression>, ModelBuildingError> {
    fn function_elem_path(
        lead_path: Vec<String>,
        function_param_name: String,
        expr: InputAccessPrimitiveExpression,
    ) -> Result<InputAccessPrimitiveExpression, ModelBuildingError> {
        match expr {
            InputAccessPrimitiveExpression::Path(function_path, parameter_name) => {
                let new_path = if parameter_name == Some(function_param_name) {
                    lead_path.clone().into_iter().chain(function_path).collect()
                } else {
                    function_path
                };
                Ok(InputAccessPrimitiveExpression::Path(
                    new_path,
                    parameter_name,
                ))
            }
            InputAccessPrimitiveExpression::Function(_, _) => Err(ModelBuildingError::Generic(
                "Cannot have a function call inside another function call".to_string(),
            )),
            expr => Ok(expr),
        }
    }

    match function_expr {
        AccessPredicateExpression::LogicalOp(op) => match op {
            AccessLogicalExpression::Not(p) => {
                let updated_expr = compute_input_function_expr(lead_path, function_param_name, *p)?;
                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Not(Box::new(updated_expr)),
                ))
            }
            AccessLogicalExpression::And(left, right) => {
                let updated_left = compute_input_function_expr(
                    lead_path.clone(),
                    function_param_name.clone(),
                    *left,
                )?;
                let updated_right =
                    compute_input_function_expr(lead_path, function_param_name, *right)?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::And(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
            AccessLogicalExpression::Or(left, right) => {
                let updated_left = compute_input_function_expr(
                    lead_path.clone(),
                    function_param_name.clone(),
                    *left,
                )?;
                let updated_right =
                    compute_input_function_expr(lead_path, function_param_name, *right)?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Or(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
        },
        AccessPredicateExpression::RelationalOp(op) => {
            let combiner = op.combiner();
            let (left, right) = op.owned_sides();

            let updated_left =
                function_elem_path(lead_path.clone(), function_param_name.clone(), *left)?;
            let updated_right = function_elem_path(lead_path, function_param_name, *right)?;
            Ok(AccessPredicateExpression::RelationalOp(combiner(
                Box::new(updated_left),
                Box::new(updated_right),
            )))
        }
        AccessPredicateExpression::BooleanLiteral(value) => {
            Ok(AccessPredicateExpression::BooleanLiteral(value))
        }
    }
}

fn compute_primitive_db_expr(
    expr: &AstExpr<Typed>,
    self_type_info: &EntityType,
    resolved_env: &ResolvedTypeEnv,
    function_context: HashMap<String, &EntityType>,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
    database: &Database,
) -> Result<DatabaseAccessPrimitiveExpression, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            let column_selection = compute_column_selection(
                selection,
                self_type_info,
                resolved_env,
                function_context,
                subsystem_primitive_types,
                subsystem_entity_types,
                database,
            )?;

            Ok(match column_selection {
                DatabasePathSelection::Column(column_path, _, parameter_name) => {
                    DatabaseAccessPrimitiveExpression::Column(column_path, parameter_name)
                }
                DatabasePathSelection::Function(column_path, function_call) => {
                    DatabaseAccessPrimitiveExpression::Function(column_path, function_call)
                }
                DatabasePathSelection::Context(c, _) => DatabaseAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::ContextSelection(c),
                ),
            })
        }
        AstExpr::StringLiteral(value, _) => Ok(DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::StringLiteral(value.clone()),
        )),
        AstExpr::BooleanLiteral(value, _) => Ok(DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::BooleanLiteral(*value),
        )),
        AstExpr::NumberLiteral(value, _) => Ok(DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NumberLiteral(*value),
        )),
        AstExpr::NullLiteral(_) => Ok(DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NullLiteral,
        )),
        AstExpr::StringList(_, _) => Err(ModelBuildingError::Generic(
            "Access expressions do not support lists yet".to_string(),
        )),
        AstExpr::LogicalOp(_) => unreachable!(), // Parser ensures that the two sides are primitive expressions
        AstExpr::RelationalOp(_) => unreachable!(), // Parser ensures that the two sides are primitive expressions
    }
}

fn compute_primitive_json_expr(
    expr: &AstExpr<Typed>,
    scope: HashMap<String, &EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
) -> Result<InputAccessPrimitiveExpression, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            let json_selection = compute_json_selection(
                selection,
                scope,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
            )?;

            Ok(match json_selection {
                JsonPathSelection::Path(path, _, parameter_name) => {
                    InputAccessPrimitiveExpression::Path(path, parameter_name)
                }
                JsonPathSelection::Context(c, _) => InputAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::ContextSelection(c),
                ),
                JsonPathSelection::Function(path, function_call) => {
                    InputAccessPrimitiveExpression::Function(path, function_call)
                }
            })
        }
        AstExpr::StringLiteral(value, _) => Ok(InputAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::StringLiteral(value.clone()),
        )),
        AstExpr::BooleanLiteral(value, _) => Ok(InputAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::BooleanLiteral(*value),
        )),
        AstExpr::NumberLiteral(value, _) => Ok(InputAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NumberLiteral(*value),
        )),
        AstExpr::NullLiteral(_) => Ok(InputAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NullLiteral,
        )),
        AstExpr::StringList(_, _) => Err(ModelBuildingError::Generic(
            "Access expressions do not support lists yet".to_string(),
        )),
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

fn compute_relational_op<PrimExpr: Send + Sync>(
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

fn compute_column_selection<'a>(
    selection: &FieldSelection<Typed>,
    self_type_info: &'a EntityType,
    resolved_env: &'a ResolvedTypeEnv<'a>,
    function_context: HashMap<String, &'a EntityType>,
    subsystem_primitive_types: &'a MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &'a MappedArena<EntityType>,
    database: &Database,
) -> Result<DatabasePathSelection<'a>, ModelBuildingError> {
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

    let path = selection.path();
    let (path_head, path_tail) = path.split_first().unwrap(); // Parser ensures that the path is not empty

    #[allow(clippy::type_complexity)]
    let compute_column_path = |lead_type: &'a EntityType,
                               selection_elems: &[FieldSelectionElement<Typed>]|
     -> (
        Option<&'a EntityType>,
        Option<PhysicalColumnPath>,
        Option<&'a FieldType<PostgresFieldType<EntityType>>>,
    ) {
        selection_elems.iter().fold(
            (Some(lead_type), None::<PhysicalColumnPath>, None),
            |(lead_type, column_path, _field_type), selection_elem| {
                let lead_type = lead_type.expect("Type for the access selection is not defined");

                match selection_elem {
                    FieldSelectionElement::Identifier(field_name, _, _) => {
                        let (field_column_path, field_type) =
                            get_column(field_name, lead_type, database);

                        let field_composite_type = match base_type(
                            field_type,
                            subsystem_primitive_types.values_ref(),
                            subsystem_entity_types.values_ref(),
                        ) {
                            PostgresType::Composite(composite_type) => Some(composite_type),
                            _ => None,
                        };

                        let new_column_path = match column_path {
                            Some(column_path) => column_path.push(field_column_path),
                            None => PhysicalColumnPath::init(field_column_path),
                        };
                        (
                            field_composite_type,
                            Some(new_column_path),
                            Some(field_type),
                        )
                    }
                    FieldSelectionElement::HofCall { .. }
                    | FieldSelectionElement::NormalCall { .. } => unreachable!(),
                }
            },
        )
    };

    match path_head {
        FieldSelectionElement::Identifier(value, _, _) => {
            if value == "self" || function_context.contains_key(value) {
                let (lead_type, parameter_name) = if value == "self" {
                    (&self_type_info, Option::<String>::None)
                } else {
                    (function_context.get(value).unwrap(), Some(value.clone()))
                };

                let (tail_last, tail_init) =
                    path_tail
                        .split_last()
                        .ok_or(ModelBuildingError::Generic(format!(
                            "Unexpected expression in @access annotation: '{value}'"
                        )))?;

                match tail_last {
                    FieldSelectionElement::Identifier(_, _, _) => {
                        let (_, column_path, field_type) =
                            compute_column_path(lead_type, path_tail);
                        // TODO: Avoid this unwrap (parser should have caught expression "self" without any fields)
                        Ok(DatabasePathSelection::Column(
                            column_path.unwrap(),
                            field_type.unwrap(),
                            parameter_name,
                        ))
                    }
                    FieldSelectionElement::HofCall {
                        name,
                        param_name: elem_name,
                        expr,
                        ..
                    } => {
                        let (field_composite_type, column_path, _field_type) =
                            compute_column_path(lead_type, tail_init);
                        let mut new_function_context = function_context.clone();
                        new_function_context
                            .extend([(elem_name.0.clone(), field_composite_type.unwrap())]);
                        let predicate_expr = compute_predicate_expression(
                            expr,
                            self_type_info,
                            new_function_context,
                            resolved_env,
                            subsystem_primitive_types,
                            subsystem_entity_types,
                            database,
                        )?;

                        Ok(DatabasePathSelection::Function(
                            column_path.unwrap(),
                            FunctionCall {
                                name: name.0.clone(),
                                parameter_name: elem_name.0.clone(),
                                expr: predicate_expr,
                            },
                        ))
                    }
                    FieldSelectionElement::NormalCall { span, .. } => {
                        Err(ModelBuildingError::Diagnosis(vec![Diagnostic {
                            level: Level::Error,
                            message: "Function calls supported only on context fields".to_string(),
                            code: Some("C000".to_string()),
                            spans: vec![SpanLabel {
                                span: *span,
                                style: SpanStyle::Primary,
                                label: None,
                            }],
                        }]))
                    }
                }
            } else {
                let (context_selection, context_field_type) = selection
                    .get_context(resolved_env.contexts, resolved_env.function_definitions)?;
                Ok(DatabasePathSelection::Context(
                    context_selection,
                    context_field_type,
                ))
            }
        }
        FieldSelectionElement::HofCall { .. } | FieldSelectionElement::NormalCall { .. } => {
            Err(ModelBuildingError::Generic(
                "Function selection at the top level is not supported".to_string(),
            ))
        }
    }
}

fn compute_json_selection<'a>(
    selection: &FieldSelection<Typed>,
    scope: HashMap<String, &'a EntityType>,
    resolved_env: &'a ResolvedTypeEnv<'a>,
    subsystem_primitive_types: &'a MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &'a MappedArena<EntityType>,
) -> Result<JsonPathSelection<'a>, ModelBuildingError> {
    fn get_field<'a>(
        field_name: &str,
        self_type_info: &'a EntityType,
    ) -> &'a PostgresField<EntityType> {
        self_type_info
            .field_by_name(field_name)
            .unwrap_or_else(|| panic!("Field {field_name} not found while processing access rules"))
    }

    let path = selection.path();
    let (path_head, path_tail) = path.split_first().unwrap(); // Parser ensures that the path is not empty

    #[allow(clippy::type_complexity)]
    let compute_json_path = |lead_type: &'a EntityType,
                             selection_elems: &[FieldSelectionElement<Typed>]|
     -> (
        Option<&'a EntityType>,
        Vec<String>,
        Option<&'a FieldType<PostgresFieldType<EntityType>>>,
    ) {
        selection_elems.iter().fold(
            (Some(lead_type), Vec::new(), None),
            |(lead_type, json_path, _field_type), selection_elem| {
                let lead_type = lead_type.expect("Type for the access selection is not defined");

                match selection_elem {
                    FieldSelectionElement::Identifier(field_name, _, _) => {
                        let field = get_field(field_name, lead_type);
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
                    }
                    FieldSelectionElement::HofCall { .. }
                    | FieldSelectionElement::NormalCall { .. } => unreachable!(),
                }
            },
        )
    };

    match path_head {
        FieldSelectionElement::Identifier(value, _, _) => {
            let scope_type = scope.get(value);

            match scope_type {
                Some(scope_type) => {
                    let parameter_name = if value == "self" {
                        Option::<String>::None
                    } else {
                        Some(value.clone())
                    };
                    let (tail_last, tail_init) =
                        path_tail
                            .split_last()
                            .ok_or(ModelBuildingError::Generic(format!(
                                "Unexpected expression in @access annotation: '{value}'"
                            )))?;

                    match tail_last {
                        FieldSelectionElement::Identifier(_, _, _) => {
                            let (_, json_path, field_type) =
                                compute_json_path(scope_type, path_tail);
                            Ok(JsonPathSelection::Path(
                                json_path,
                                field_type.unwrap(),
                                parameter_name,
                            ))
                        }
                        FieldSelectionElement::HofCall {
                            name,
                            param_name: elem_name,
                            expr,
                            ..
                        } => {
                            let (field_composite_type, path, _field_type) =
                                compute_json_path(scope_type, tail_init);
                            let mut new_function_context = scope.clone();
                            new_function_context
                                .extend([(elem_name.0.clone(), field_composite_type.unwrap())]);
                            let predicate_expr = compute_input_predicate_expression(
                                expr,
                                new_function_context,
                                resolved_env,
                                subsystem_primitive_types,
                                subsystem_entity_types,
                            )?;

                            Ok(JsonPathSelection::Function(
                                path,
                                FunctionCall {
                                    name: name.0.clone(),
                                    parameter_name: elem_name.0.clone(),
                                    expr: predicate_expr,
                                },
                            ))
                        }
                        FieldSelectionElement::NormalCall { span, .. } => {
                            Err(ModelBuildingError::Diagnosis(vec![Diagnostic {
                                level: Level::Error,
                                message: "Function calls supported only on context fields"
                                    .to_string(),
                                code: Some("C000".to_string()),
                                spans: vec![SpanLabel {
                                    span: *span,
                                    style: SpanStyle::Primary,
                                    label: None,
                                }],
                            }]))
                        }
                    }
                }
                None => {
                    let (context_selection, context_field_type) = selection
                        .get_context(resolved_env.contexts, resolved_env.function_definitions)?;
                    Ok(JsonPathSelection::Context(
                        context_selection,
                        context_field_type,
                    ))
                }
            }
        }
        FieldSelectionElement::HofCall { .. } | FieldSelectionElement::NormalCall { .. } => {
            Err(ModelBuildingError::Generic(
                "Function selection at the top level is not supported".to_string(),
            ))
        }
    }
}

enum NestedPredicatePart<T> {
    // Uses only the parent elements
    Parent(T),
    // Uses only the nested elements
    Nested(T),
    // Constants, context selection etc
    Common(T),
}

/// Compute the predicate that should be applied to the parent entity
///
/// This works in conjunction with `TransactionStep::Filter` step to narrows down parent elements
/// for which we need to perform a nested operation.
///
/// For example, assume that the access predicate for the `Document` is `self.user.id =
/// AuthContext.id`, and the parent entity is `User`. This function will compute a predicate for
/// `User` to be `self.id = `AuthContext.id`.
///
/// Implementation note: We examine each constituent of the predicate expression and determine if it
/// uses only the parent elements, only the nested elements, or both. Then depending on the element
/// used, we construct a predicate that can be applied to the parent entity.
pub fn parent_predicate(
    expr: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    parent_entity: &EntityType,
) -> Result<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>, ModelBuildingError> {
    let reduced = reduce_nested_predicate(expr, parent_entity)?;

    Ok(match reduced {
        NestedPredicatePart::Parent(expr) => expr,
        _ => AccessPredicateExpression::BooleanLiteral(true),
    })
}

fn reduce_nested_predicate(
    expr: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    parent_entity: &EntityType,
) -> Result<
    NestedPredicatePart<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    ModelBuildingError,
> {
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
) -> Result<
    NestedPredicatePart<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    ModelBuildingError,
> {
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
        ModelBuildingError,
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
) -> Result<
    NestedPredicatePart<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>>,
    ModelBuildingError,
> {
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
        ModelBuildingError,
    > {
        let reduced_lhs = reduce_nested_primitive_expr(lhs, parent_entity);
        let reduced_rhs = reduce_nested_primitive_expr(rhs, parent_entity);

        match (reduced_lhs, reduced_rhs) {
            (NestedPredicatePart::Parent(l), NestedPredicatePart::Parent(r))
            | (NestedPredicatePart::Common(l), NestedPredicatePart::Parent(r))
            | (NestedPredicatePart::Parent(l), NestedPredicatePart::Common(r)) => {
                Ok(NestedPredicatePart::Parent(AccessPredicateExpression::RelationalOp(combiner(
                    Box::new(l),
                    Box::new(r),
                ))))
            }
            (NestedPredicatePart::Nested(l), NestedPredicatePart::Nested(r))
            | (NestedPredicatePart::Common(l), NestedPredicatePart::Nested(r))
            | (NestedPredicatePart::Nested(l), NestedPredicatePart::Common(r)) => {
                Ok(NestedPredicatePart::Nested(AccessPredicateExpression::RelationalOp(combiner(
                    Box::new(l),
                    Box::new(r),
                ))))
            }
            (NestedPredicatePart::Common(l), NestedPredicatePart::Common(r)) => {
                Ok(NestedPredicatePart::Common(AccessPredicateExpression::RelationalOp(combiner(
                    Box::new(l),
                    Box::new(r),
                ))))
            }
            (NestedPredicatePart::Parent(_), NestedPredicatePart::Nested(_)) |
            (NestedPredicatePart::Nested(_), NestedPredicatePart::Parent(_)) => {
                Err(ModelBuildingError::Generic(
                    "Access expression comparing a parent field with a nested field is not supported".to_string(),
                ))
            }
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
                    // Eliminate the head link. For example if the expression is self.user.id, then
                    // we can reduce it to just id (assuming that the parent entity is user)
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
