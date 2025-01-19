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
        ast::ast_types::{AstExpr, FieldSelection},
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

use postgres_core_model::types::{
    base_type, EntityType, PostgresFieldType, PostgresPrimitiveType, PostgresType,
};
use postgres_core_model::{access::InputAccessPrimitiveExpression, types::PostgresField};

use crate::resolved_type::ResolvedTypeEnv;

use super::common::{compute_logical_op, compute_relational_op};

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
