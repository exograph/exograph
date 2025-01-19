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

use exo_sql::{Database, PhysicalColumnPath};
use postgres_core_model::{
    access::{AccessPrimitiveExpressionPath, FieldPath, PrecheckAccessPrimitiveExpression},
    relation::PostgresRelation,
    types::{base_type, EntityType, PostgresFieldType, PostgresPrimitiveType, PostgresType},
};

use serde::Serialize;

use crate::resolved_type::ResolvedTypeEnv;

use super::common::{compute_logical_op, compute_relational_op};

#[derive(Serialize, Debug)]
enum PrecheckPathSelection<'a> {
    Path(
        AccessPrimitiveExpressionPath,
        &'a FieldType<PostgresFieldType<EntityType>>,
        Option<String>,
    ),
    Function(
        AccessPrimitiveExpressionPath,
        FunctionCall<PrecheckAccessPrimitiveExpression>,
    ),
    Context(ContextSelection, &'a ContextFieldType),
}

pub fn compute_precheck_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: &EntityType,
    function_context: HashMap<String, &EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
    database: &Database,
) -> Result<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            let selection = compute_precheck_selection(
                selection,
                self_type_info,
                function_context,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
                database,
            )?;

            match selection {
                PrecheckPathSelection::Path(path, field_type, parameter_name) => {
                    let field_entity_type = field_type.innermost().type_id.to_type(
                        subsystem_primitive_types.values_ref(),
                        subsystem_entity_types.values_ref(),
                    );

                    if field_entity_type.name() == "Boolean" {
                        // Treat boolean context expressions in the same way as an "eq" relational expression
                        // For example, treat `AuthContext.superUser` the same way as `AuthContext.superUser == true`
                        Ok(AccessPredicateExpression::RelationalOp(
                            AccessRelationalOp::Eq(
                                Box::new(PrecheckAccessPrimitiveExpression::Path(
                                    path,
                                    parameter_name,
                                )),
                                Box::new(PrecheckAccessPrimitiveExpression::Common(
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
                PrecheckPathSelection::Context(context_selection, field_type) => {
                    if field_type.innermost() == &PrimitiveType::Boolean {
                        // Treat boolean context expressions in the same way as an "eq" relational expression
                        // For example, treat `AuthContext.superUser` the same way as `AuthContext.superUser == true`
                        Ok(AccessPredicateExpression::RelationalOp(
                            AccessRelationalOp::Eq(
                                Box::new(PrecheckAccessPrimitiveExpression::Common(
                                    CommonAccessPrimitiveExpression::ContextSelection(
                                        context_selection,
                                    ),
                                )),
                                Box::new(PrecheckAccessPrimitiveExpression::Common(
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

                PrecheckPathSelection::Function(path, function_call) => {
                    compute_precheck_function_expr(
                        path,
                        function_call.parameter_name,
                        function_call.expr,
                    )
                }
            }
        }
        AstExpr::LogicalOp(op) => {
            let predicate_expr = |expr: &AstExpr<Typed>| {
                compute_precheck_predicate_expression(
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
            let primitive_expr = |expr: &AstExpr<Typed>| {
                compute_primitive_precheck_expr(
                    expr,
                    self_type_info,
                    function_context.clone(),
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
                    database,
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

fn compute_precheck_function_expr(
    path: AccessPrimitiveExpressionPath,
    function_param_name: String,
    function_expr: AccessPredicateExpression<PrecheckAccessPrimitiveExpression>,
) -> Result<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>, ModelBuildingError> {
    fn function_elem_path(
        lead_path: AccessPrimitiveExpressionPath,
        function_param_name: String,
        expr: PrecheckAccessPrimitiveExpression,
    ) -> Result<PrecheckAccessPrimitiveExpression, ModelBuildingError> {
        match expr {
            PrecheckAccessPrimitiveExpression::Path(function_column_path, parameter_name) => {
                // We may have expression like `self.documentUser.some(du => du.read)`, in which case we want to join the column path
                // to form `self.documentUser.read`.
                //
                // However, if the lead path is `self.documentUser.some(du => du.id === self.id)`, we don't want to join the column path
                // for the `self.id` part.
                Ok(PrecheckAccessPrimitiveExpression::Path(
                    if parameter_name == Some(function_param_name) {
                        lead_path.join(function_column_path)?
                    } else {
                        function_column_path
                    },
                    parameter_name,
                ))
            }
            PrecheckAccessPrimitiveExpression::Function(_, _) => Err(ModelBuildingError::Generic(
                "Cannot have a function call inside another function call".to_string(),
            )),
            PrecheckAccessPrimitiveExpression::Common(..) => Ok(expr),
        }
    }

    match function_expr {
        AccessPredicateExpression::LogicalOp(op) => match op {
            AccessLogicalExpression::Not(p) => {
                let updated_expr = compute_precheck_function_expr(path, function_param_name, *p)?;
                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Not(Box::new(updated_expr)),
                ))
            }
            AccessLogicalExpression::And(left, right) => {
                let updated_left = compute_precheck_function_expr(
                    path.clone(),
                    function_param_name.clone(),
                    *left,
                )?;
                let updated_right =
                    compute_precheck_function_expr(path, function_param_name, *right)?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::And(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
            AccessLogicalExpression::Or(left, right) => {
                let updated_left = compute_precheck_function_expr(
                    path.clone(),
                    function_param_name.clone(),
                    *left,
                )?;
                let updated_right =
                    compute_precheck_function_expr(path, function_param_name, *right)?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Or(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
        },
        AccessPredicateExpression::RelationalOp(op) => {
            let combiner = op.combiner();
            let (left, right) = op.owned_sides();

            let updated_left =
                function_elem_path(path.clone(), function_param_name.clone(), *left)?;
            let updated_right = function_elem_path(path, function_param_name, *right)?;
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

fn compute_primitive_precheck_expr(
    expr: &AstExpr<Typed>,
    self_type_info: &EntityType,
    function_context: HashMap<String, &EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
    database: &Database,
) -> Result<PrecheckAccessPrimitiveExpression, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(field_selection) => {
            let selection = compute_precheck_selection(
                field_selection,
                self_type_info,
                function_context,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
                database,
            )?;

            Ok(match selection {
                PrecheckPathSelection::Path(path, _, parameter_name) => {
                    PrecheckAccessPrimitiveExpression::Path(path, parameter_name)
                }
                PrecheckPathSelection::Function(path, function_call) => {
                    PrecheckAccessPrimitiveExpression::Function(path, function_call)
                }
                PrecheckPathSelection::Context(c, _) => PrecheckAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::ContextSelection(c),
                ),
            })
        }
        AstExpr::StringLiteral(value, _) => Ok(PrecheckAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::StringLiteral(value.clone()),
        )),
        AstExpr::BooleanLiteral(value, _) => Ok(PrecheckAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::BooleanLiteral(*value),
        )),
        AstExpr::NumberLiteral(value, _) => Ok(PrecheckAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NumberLiteral(*value),
        )),
        AstExpr::NullLiteral(_) => Ok(PrecheckAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NullLiteral,
        )),
        AstExpr::StringList(_, _) => Err(ModelBuildingError::Generic(
            "Access expressions do not support lists yet".to_string(),
        )),
        AstExpr::LogicalOp(_) => unreachable!(), // Parser ensures that the two sides are primitive expressions
        AstExpr::RelationalOp(_) => unreachable!(), // Parser ensures that the two sides are primitive expressions
    }
}

fn compute_precheck_selection<'a>(
    selection: &FieldSelection<Typed>,
    self_type_info: &'a EntityType,
    function_context: HashMap<String, &'a EntityType>,
    resolved_env: &'a ResolvedTypeEnv<'a>,
    subsystem_primitive_types: &'a MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &'a MappedArena<EntityType>,
    database: &Database,
) -> Result<PrecheckPathSelection<'a>, ModelBuildingError> {
    #[allow(clippy::type_complexity)]
    let compute_path = |lead_type: &'a EntityType,
                        selection_elems: &[FieldSelectionElement<Typed>]|
     -> Result<
        (
            Option<&'a EntityType>,
            Option<AccessPrimitiveExpressionPath>,
            Option<&'a FieldType<PostgresFieldType<EntityType>>>,
        ),
        ModelBuildingError,
    > {
        let (lead_type, path, field_type, _) = selection_elems.iter().try_fold(
            (
                Some(lead_type),
                None::<AccessPrimitiveExpressionPath>,
                None,
                false,
            ),
            |(lead_type, path, _field_type, in_many_to_one), selection_elem| {
                let lead_type = lead_type.expect("Type for the access selection is not defined");

                match selection_elem {
                    FieldSelectionElement::Identifier(field_name, _, _) => {
                        let field = lead_type.field_by_name(field_name).unwrap_or_else(|| {
                            panic!("Field {field_name} not found while processing access rules")
                        });
                        let field_relation = &field.relation;
                        let field_type = &field.typ;
                        let field_column_path = field.relation.column_path_link(database);

                        let field_composite_type = match base_type(
                            field_type,
                            subsystem_primitive_types.values_ref(),
                            subsystem_entity_types.values_ref(),
                        ) {
                            PostgresType::Composite(composite_type) => Some(composite_type),
                            _ => None,
                        };

                        let new_path = match path {
                            Some(AccessPrimitiveExpressionPath {
                                column_path,
                                field_path,
                            }) => {
                                let column_path = column_path.push(field_column_path);

                                let field_path =
                                    match (field_path, !in_many_to_one || field_relation.is_pk()) {
                                        (FieldPath::Normal(a), true) => {
                                            let mut field_path = a.clone();
                                            field_path.push(field_name.clone());
                                            FieldPath::Normal(field_path)
                                        }
                                        (FieldPath::Normal(a), false) => FieldPath::Pk {
                                            lead: a.clone(),
                                            pk_fields: lead_type
                                                .pk_fields()
                                                .iter()
                                                .map(|f| f.name.clone())
                                                .collect(),
                                        },
                                        (field_path, _) => {
                                            // If the field path is already a pk, we leave it as is (will lead to a database residue)
                                            field_path
                                        }
                                    };

                                AccessPrimitiveExpressionPath {
                                    column_path,
                                    field_path,
                                }
                            }
                            None => AccessPrimitiveExpressionPath::new(
                                PhysicalColumnPath::init(field_column_path),
                                FieldPath::Normal(vec![field_name.clone()]),
                            ),
                        };

                        Ok((
                            field_composite_type,
                            Some(new_path),
                            Some(field_type),
                            in_many_to_one
                                || matches!(field_relation, PostgresRelation::ManyToOne { .. }),
                        ))
                    }
                    FieldSelectionElement::HofCall { .. }
                    | FieldSelectionElement::NormalCall { .. } => Err(ModelBuildingError::Generic(
                        "Function calls supported only on context fields".to_string(),
                    )),
                }
            },
        )?;

        Ok((lead_type, path, field_type))
    };

    let path = selection.path();
    let (path_head, path_tail) = path.split_first().unwrap(); // Parser ensures that the path is not empty

    match path_head {
        FieldSelectionElement::Identifier(value, _, _) => {
            if value == "self" || function_context.contains_key(value) {
                let (lead_type, parameter_name) = if value == "self" {
                    (&self_type_info, Option::<String>::None)
                } else {
                    (function_context.get(value).unwrap(), Some(value.clone()))
                };

                // The last element could be an ordinary field or a function call
                let (tail_last, tail_init) =
                    path_tail
                        .split_last()
                        .ok_or(ModelBuildingError::Generic(format!(
                            "Unexpected expression in @access annotation: '{value}'"
                        )))?;

                match tail_last {
                    FieldSelectionElement::Identifier(_, _, _) => {
                        let (_, column_path, field_type) = compute_path(lead_type, path_tail)?;
                        // TODO: Avoid these unwrap (parser should have caught expression "self" without any fields)
                        Ok(PrecheckPathSelection::Path(
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
                            compute_path(lead_type, tail_init)?;
                        let mut new_function_context = function_context.clone();
                        new_function_context
                            .extend([(elem_name.0.clone(), field_composite_type.unwrap())]);
                        let predicate_expr = compute_precheck_predicate_expression(
                            expr,
                            self_type_info,
                            new_function_context,
                            resolved_env,
                            subsystem_primitive_types,
                            subsystem_entity_types,
                            database,
                        )?;
                        Ok(PrecheckPathSelection::Function(
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
                Ok(PrecheckPathSelection::Context(
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

#[cfg(test)]
mod tests {
    use codemap::{CodeMap, Span};
    use core_model_builder::{
        ast::ast_types::{Identifier, RelationalOp},
        typechecker::typ::Type,
    };

    use crate::{
        test_util::{
            create_base_model_system, create_postgres_core_subsystem,
            create_typechecked_system_from_src,
        },
        SystemContextBuilding,
    };

    use super::*;

    const ISSUE_TRACKING_SRC: &str = "
        context AuthContext {
            @jwt title: String
        }

        @postgres
        module IssueDatabase {
            @access(true)
            type Issue {
                @pk id: Int = autoIncrement()
                title: String
                assignee: Employee
            }

            @access(true)
            type Employee {
                @pk id: Int = autoIncrement()
                name: String
                position: String
                issues: Set<Issue>?
            }
        }
    ";

    #[test]
    fn direct_field() -> Result<(), ModelBuildingError> {
        let selection = create_field_selection("self.id");
        assert_precheck_selection(selection, "Issue", "direct_field")
    }

    #[test]
    fn many_to_one_pk_field() -> Result<(), ModelBuildingError> {
        let selection = create_field_selection("self.assignee.id");
        assert_precheck_selection(selection, "Issue", "many_to_one_pk_field")
    }

    #[test]
    fn many_to_one_non_pk_field() -> Result<(), ModelBuildingError> {
        let selection = create_field_selection("self.assignee.position");
        assert_precheck_selection(selection, "Issue", "many_to_one_non_pk_field")
    }

    #[test]
    fn hof_call() -> Result<(), ModelBuildingError> {
        // self.issues.some(i => i.title == AuthContext.title)
        let self_issues_selection = create_field_selection("self.issues");

        let hof_elem = FieldSelectionElement::HofCall {
            span: null_span(),
            name: Identifier("some".to_string(), null_span()),
            param_name: Identifier("i".to_string(), null_span()),
            expr: Box::new(AstExpr::RelationalOp(RelationalOp::Eq(
                Box::new(AstExpr::FieldSelection(create_field_selection("i.title"))),
                Box::new(AstExpr::FieldSelection(create_field_selection(
                    "AuthContext.title",
                ))),
                Type::Defer,
            ))),
            typ: Type::Defer,
        };

        let selection = FieldSelection::Select(
            Box::new(self_issues_selection),
            hof_elem,
            null_span(),
            Type::Defer,
        );

        assert_precheck_selection(selection, "Employee", "hof_call")
    }

    fn assert_precheck_selection(
        selection: FieldSelection<Typed>,
        entity_name: &str,
        test_name: &str,
    ) -> Result<(), ModelBuildingError> {
        let typechecked_system = create_typechecked_system_from_src(ISSUE_TRACKING_SRC)?;
        let resolved_types = crate::resolved_builder::build(&typechecked_system)?;

        let base_system = create_base_model_system(&typechecked_system)?;
        let system = create_postgres_core_subsystem(&base_system, &typechecked_system)?;

        let resolved_env = ResolvedTypeEnv {
            contexts: &base_system.contexts,
            resolved_types,
            function_definitions: &base_system.function_definitions,
        };

        let database = &system.database;

        let entity_type = get_entity_type(&system, entity_name);

        let selection = compute_precheck_selection(
            &selection,
            entity_type,
            HashMap::new(),
            &resolved_env,
            &system.primitive_types,
            &system.entity_types,
            database,
        )?;

        insta::assert_yaml_snapshot!(test_name, selection);

        Ok(())
    }

    fn create_field_selection(access_expr: &str) -> FieldSelection<Typed> {
        // Currently we assume simple expressions like `self.id` or `self.user.id` (i.e. not involve HOFs)
        let split = access_expr.rsplit_once('.');

        match split {
            None => FieldSelection::Single(
                FieldSelectionElement::Identifier(
                    access_expr.to_string(),
                    null_span(),
                    Type::Defer,
                ),
                Type::Defer,
            ),
            Some((prefix, suffix)) => {
                if suffix.is_empty() {
                    create_field_selection(prefix)
                } else {
                    let prefix_selection = create_field_selection(prefix);

                    FieldSelection::Select(
                        Box::new(prefix_selection),
                        FieldSelectionElement::Identifier(
                            suffix.to_string(),
                            null_span(),
                            Type::Defer,
                        ),
                        null_span(),
                        Type::Defer,
                    )
                }
            }
        }
    }

    fn get_entity_type<'a>(
        postgres_core_subsystem: &'a SystemContextBuilding,
        entity_name: &str,
    ) -> &'a EntityType {
        postgres_core_subsystem
            .entity_types
            .get_by_key(entity_name)
            .unwrap()
    }

    fn null_span() -> Span {
        let mut codemap = CodeMap::new();
        let file = codemap.add_file("".to_string(), "".to_string());
        file.span
    }
}
