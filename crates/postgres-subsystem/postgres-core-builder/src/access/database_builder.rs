// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    collections::{
        HashMap,
        hash_map::{DefaultHasher, Entry},
    },
    hash::{Hash, Hasher},
};

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::{
    access::{
        AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
        CommonAccessPrimitiveExpression, FunctionCall,
    },
    context_type::{ContextFieldType, ContextSelection},
    mapped_arena::MappedArena,
    primitive_type::{self, PrimitiveType},
    types::FieldType,
};
use core_model_builder::{
    ast::ast_types::{AstExpr, FieldSelection, FieldSelectionElement},
    error::ModelBuildingError,
    typechecker::Typed,
};

use exo_sql::{ColumnPathLink, Database, PhysicalColumnPath};
use postgres_core_model::access::DatabaseAccessPrimitiveExpression;
use postgres_core_model::types::{
    EntityType, PostgresFieldType, PostgresPrimitiveType, PostgresType, base_type,
};

use crate::resolved_type::ResolvedTypeEnv;

use super::common::{compute_logical_op, compute_relational_op};

#[derive(Clone)]
pub struct FunctionContextEntry<'a> {
    entity_type: Option<&'a EntityType>,
    path: Option<PhysicalColumnPath>,
}

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

pub fn compute_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: &EntityType,
    function_context: HashMap<String, FunctionContextEntry>,
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
                function_context.clone(),
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
                        == primitive_type::BooleanType::NAME
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
                        let mut updated_context = function_context.clone();
                        match updated_context.entry(function_call.parameter_name.clone()) {
                            Entry::Occupied(mut entry) => {
                                entry.get_mut().path = Some(column_path.clone());
                            }
                            Entry::Vacant(entry) => {
                                entry.insert(FunctionContextEntry {
                                    entity_type: None,
                                    path: Some(column_path.clone()),
                                });
                            }
                        }

                        compute_function_expr(
                            column_path,
                            function_call.parameter_name,
                            function_call.expr,
                            updated_context,
                        )
                    }
                }
                DatabasePathSelection::Context(context_selection, field_type) => {
                    if field_type.innermost() == &PrimitiveType::Plain(primitive_type::BOOLEAN_TYPE)
                    {
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

fn compute_primitive_db_expr(
    expr: &AstExpr<Typed>,
    self_type_info: &EntityType,
    resolved_env: &ResolvedTypeEnv,
    function_context: HashMap<String, FunctionContextEntry>,
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
            CommonAccessPrimitiveExpression::NumberLiteral(value.clone()),
        )),
        AstExpr::NullLiteral(_) => Ok(DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::NullLiteral,
        )),
        AstExpr::StringList(_, _) => Err(ModelBuildingError::Generic(
            "Access expressions do not support lists yet".to_string(),
        )),
        AstExpr::ObjectLiteral(_, _) => Err(ModelBuildingError::Generic(
            "Access expressions do not support object literals".to_string(),
        )),
        AstExpr::LogicalOp(_) => unreachable!(), // Parser ensures that the two sides are primitive expressions
        AstExpr::RelationalOp(_) => unreachable!(), // Parser ensures that the two sides are primitive expressions
    }
}

fn compute_column_selection<'a>(
    selection: &FieldSelection<Typed>,
    self_type_info: &'a EntityType,
    resolved_env: &'a ResolvedTypeEnv<'a>,
    function_context: HashMap<String, FunctionContextEntry<'a>>,
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
                panic!("Field '{field_name}' not found while processing access rules")
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
                               base_path: Option<PhysicalColumnPath>,
                               selection_elems: &[FieldSelectionElement<Typed>]|
     -> (
        Option<&'a EntityType>,
        Option<PhysicalColumnPath>,
        Option<&'a FieldType<PostgresFieldType<EntityType>>>,
    ) {
        selection_elems.iter().fold(
            (Some(lead_type), base_path, None),
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
                let (lead_type, parameter_name, base_path) = if value == "self" {
                    (self_type_info, Option::<String>::None, None)
                } else {
                    let entry = function_context.get(value).unwrap_or_else(|| {
                        panic!(
                            "Function parameter '{value}' not found while processing access rules"
                        )
                    });
                    (
                        entry
                            .entity_type
                            .expect("Function parameter type not available"),
                        Some(value.clone()),
                        entry.path.clone(),
                    )
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
                            compute_column_path(lead_type, base_path.clone(), path_tail);

                        let column_path = column_path.expect("Unexpected empty column path");

                        // If the column path points to a relation without completing the chain, substitute the relation with self column of the relation
                        // This handles cases, where an access rule such as =`self.user != null`. Here we substitute `self.user` with the column `user_id` from the `User` table
                        // TODO: Consider an alternative approach, where we have a special variant to express this
                        let (head, tail) = column_path.split_head();
                        let column_path = if tail.is_none() {
                            if let ColumnPathLink::Relation(r) = head {
                                PhysicalColumnPath::init(ColumnPathLink::Leaf(
                                    r.column_pairs[0].self_column_id,
                                ))
                            } else {
                                column_path
                            }
                        } else {
                            column_path
                        };

                        Ok(DatabasePathSelection::Column(
                            column_path,
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
                            compute_column_path(lead_type, base_path, tail_init);

                        let column_path = column_path.expect("Unexpected empty column path");
                        let column_path = ensure_parameter_alias(column_path, &elem_name.0);
                        let composite_type = field_composite_type.unwrap_or_else(|| {
                            panic!("Function call expected to operate on a collection type")
                        });

                        let mut new_function_context = function_context.clone();
                        new_function_context.insert(
                            elem_name.0.clone(),
                            FunctionContextEntry {
                                entity_type: Some(composite_type),
                                path: Some(column_path.clone()),
                            },
                        );

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
                            column_path.clone(),
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

fn ensure_parameter_alias(path: PhysicalColumnPath, parameter_name: &str) -> PhysicalColumnPath {
    let alias_base = if parameter_name.is_empty() {
        "nested"
    } else {
        parameter_name
    };

    let mut links = Vec::new();
    let mut remaining = Some(path);
    while let Some(current) = remaining {
        let (head, tail) = current.split_head();
        links.push(head);
        remaining = tail;
    }

    let mut iter = links.into_iter();
    let head = iter
        .next()
        .expect("Unexpected empty column path for function");
    let mut rebuilt = PhysicalColumnPath::init(alias_link(head, alias_base, 0));

    for (index, link) in iter.enumerate() {
        rebuilt = rebuilt.push(alias_link(link, alias_base, index + 1));
    }

    rebuilt
}

fn alias_link(link: ColumnPathLink, alias_base: &str, position: usize) -> ColumnPathLink {
    match link {
        ColumnPathLink::Relation(mut relation) => {
            if relation.linked_table_alias.is_none() {
                let mut hasher = DefaultHasher::new();
                alias_base.hash(&mut hasher);
                position.hash(&mut hasher);
                relation.hash(&mut hasher);
                let hashed = hasher.finish();
                let suffix = format!("{hashed:016x}");
                let alias = format!("{alias_base}_{}", &suffix[..8]);
                relation = relation.with_alias(alias);
            }
            ColumnPathLink::Relation(relation)
        }
        other => other,
    }
}

fn compute_function_expr(
    lead_path: PhysicalColumnPath,
    function_param_name: String,
    function_expr: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    function_context: HashMap<String, FunctionContextEntry>,
) -> Result<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>, ModelBuildingError> {
    fn normalize_single_relation(path: PhysicalColumnPath) -> PhysicalColumnPath {
        let (head, tail) = path.split_head();

        if tail.is_none() {
            if let ColumnPathLink::Relation(relation) = head {
                if relation.column_pairs.len() == 1 {
                    return PhysicalColumnPath::leaf(relation.column_pairs[0].self_column_id);
                }
            }
        }

        path
    }

    fn with_head_alias(path: PhysicalColumnPath, alias: String) -> PhysicalColumnPath {
        let (head, tail) = path.split_head();
        let new_head = match head {
            ColumnPathLink::Relation(relation) => {
                ColumnPathLink::Relation(relation.with_alias(alias))
            }
            other => other,
        };

        let mut rebuilt = PhysicalColumnPath::init(new_head);
        if let Some(tail_path) = tail {
            rebuilt = rebuilt.join(tail_path);
        }

        rebuilt
    }

    fn ensure_alias_if_conflict(
        path: PhysicalColumnPath,
        lead_path: &PhysicalColumnPath,
        parameter_name: &Option<String>,
    ) -> PhysicalColumnPath {
        if path.alias().is_some() || path.lead_table_id() != lead_path.lead_table_id() {
            return path;
        }

        let alias = parameter_name
            .as_ref()
            .map(|name| format!("{name}_path"))
            .unwrap_or_else(|| "nested_path".to_string());

        with_head_alias(path, alias)
    }

    fn function_elem_path(
        lead_path: &PhysicalColumnPath,
        function_param_name: &str,
        expr: DatabaseAccessPrimitiveExpression,
        function_context: &HashMap<String, FunctionContextEntry>,
    ) -> Result<DatabaseAccessPrimitiveExpression, ModelBuildingError> {
        match expr {
            DatabaseAccessPrimitiveExpression::Column(function_column_path, parameter_name) => {
                let resolved_path = if let Some(param_name) = parameter_name.as_deref() {
                    if param_name == function_param_name {
                        if function_column_path.lead_table_id() == lead_path.lead_table_id() {
                            normalize_single_relation(function_column_path)
                        } else {
                            normalize_single_relation(lead_path.clone().join(function_column_path))
                        }
                    } else if let Some(entry) = function_context.get(param_name) {
                        if let Some(base_path) = &entry.path {
                            if function_column_path.lead_table_id() == base_path.lead_table_id() {
                                normalize_single_relation(function_column_path)
                            } else {
                                normalize_single_relation(
                                    base_path.clone().join(function_column_path),
                                )
                            }
                        } else {
                            normalize_single_relation(function_column_path)
                        }
                    } else {
                        normalize_single_relation(function_column_path)
                    }
                } else {
                    normalize_single_relation(function_column_path)
                };

                let resolved_path =
                    ensure_alias_if_conflict(resolved_path, lead_path, &parameter_name);

                if std::env::var("EXO_DEBUG_FUNCTION_PATH").is_ok() {
                    println!(
                        "[function-debug] lead_path={lead_path:?} parameter={parameter_name:?} resolved_path={resolved_path:?}"
                    );
                }

                Ok(DatabaseAccessPrimitiveExpression::Column(
                    resolved_path,
                    parameter_name,
                ))
            }
            DatabaseAccessPrimitiveExpression::Function(nested_lead_path, nested_call) => {
                let FunctionCall {
                    name,
                    parameter_name,
                    expr,
                } = nested_call;

                let nested_lead_path = ensure_parameter_alias(nested_lead_path, &parameter_name);
                let updated_lead_column_path = lead_path.clone().join(nested_lead_path.clone());

                if std::env::var("EXO_DEBUG_FUNCTION_PATH").is_ok() {
                    println!(
                        "[function-debug] nested lead={lead_path:?} nested_path={nested_lead_path:?} updated_lead={updated_lead_column_path:?}"
                    );
                }

                let mut updated_context = function_context.clone();
                updated_context.insert(
                    parameter_name.clone(),
                    FunctionContextEntry {
                        entity_type: None,
                        path: Some(updated_lead_column_path.clone()),
                    },
                );

                let updated_expr = compute_function_expr(
                    updated_lead_column_path.clone(),
                    parameter_name.clone(),
                    expr,
                    updated_context,
                )?;

                Ok(DatabaseAccessPrimitiveExpression::Function(
                    updated_lead_column_path,
                    FunctionCall {
                        name,
                        parameter_name,
                        expr: updated_expr,
                    },
                ))
            }
            expr => Ok(expr),
        }
    }

    match function_expr {
        AccessPredicateExpression::LogicalOp(op) => match op {
            AccessLogicalExpression::Not(p) => {
                let updated_expr = compute_function_expr(
                    lead_path.clone(),
                    function_param_name.clone(),
                    *p,
                    function_context.clone(),
                )?;
                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Not(Box::new(updated_expr)),
                ))
            }
            AccessLogicalExpression::And(left, right) => {
                let updated_left = compute_function_expr(
                    lead_path.clone(),
                    function_param_name.clone(),
                    *left,
                    function_context.clone(),
                )?;
                let updated_right = compute_function_expr(
                    lead_path,
                    function_param_name,
                    *right,
                    function_context,
                )?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::And(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
            AccessLogicalExpression::Or(left, right) => {
                let updated_left = compute_function_expr(
                    lead_path.clone(),
                    function_param_name.clone(),
                    *left,
                    function_context.clone(),
                )?;
                let updated_right = compute_function_expr(
                    lead_path,
                    function_param_name,
                    *right,
                    function_context,
                )?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Or(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
        },
        AccessPredicateExpression::RelationalOp(op) => {
            let combiner = op.combiner();
            let (left, right) = op.owned_sides();

            let updated_left =
                function_elem_path(&lead_path, &function_param_name, *left, &function_context)?;
            let updated_right =
                function_elem_path(&lead_path, &function_param_name, *right, &function_context)?;
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
