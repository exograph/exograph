use core_plugin_interface::{
    core_model::{
        access::{
            AccessContextSelection, AccessLogicalExpression, AccessPredicateExpression,
            AccessRelationalOp,
        },
        context_type::ContextFieldType,
        mapped_arena::MappedArena,
        primitive_type::PrimitiveType,
        types::DecoratedType,
    },
    core_model_builder::{
        ast::ast_types::{AstExpr, FieldSelection, LogicalOp, RelationalOp},
        error::ModelBuildingError,
        typechecker::Typed,
    },
};

use postgres_model::{
    access::DatabaseAccessPrimitiveExpression,
    column_path::{ColumnIdPath, ColumnIdPathLink},
    types::{base_type, EntityType, FieldType, PostgresPrimitiveType, PostgresType},
};

use super::column_path_utils;

use super::type_builder::ResolvedTypeEnv;

enum PathSelection<'a> {
    Column(ColumnIdPath, &'a DecoratedType<FieldType<EntityType>>),
    Context(AccessContextSelection, &'a ContextFieldType),
}

pub fn compute_predicate_expression(
    expr: &AstExpr<Typed>,
    self_type_info: Option<&EntityType>,
    resolved_env: &ResolvedTypeEnv,
    subsystem_primitive_types: &MappedArena<PostgresPrimitiveType>,
    subsystem_entity_types: &MappedArena<EntityType>,
) -> Result<AccessPredicateExpression<DatabaseAccessPrimitiveExpression>, ModelBuildingError> {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_selection(
                selection,
                self_type_info,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
            ) {
                PathSelection::Column(column_path, column_type) => {
                    if base_type(
                        column_type,
                        &subsystem_primitive_types.values,
                        &subsystem_entity_types.values,
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
                    if field_type.primitive_type() == &PrimitiveType::Boolean {
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
                )),
                Box::new(compute_primitive_expr(
                    right,
                    self_type_info,
                    resolved_env,
                    subsystem_primitive_types,
                    subsystem_entity_types,
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
) -> DatabaseAccessPrimitiveExpression {
    match expr {
        AstExpr::FieldSelection(selection) => {
            match compute_selection(
                selection,
                self_type_info,
                resolved_env,
                subsystem_primitive_types,
                subsystem_entity_types,
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
        field_name: &str,
        self_type_info: &'a EntityType,
        entity_types: &MappedArena<EntityType>,
    ) -> (ColumnIdPathLink, &'a DecoratedType<FieldType<EntityType>>) {
        let get_field = |field_name: &str| {
            self_type_info.field(field_name).unwrap_or_else(|| {
                panic!("Field {field_name} not found while processing access rules")
            })
        };

        let field = get_field(field_name);
        let column_path_link =
            column_path_utils::column_path_link(self_type_info, field, entity_types);

        (column_path_link, &field.typ)
    }

    fn get_context<'a>(
        path_elements: &[String],
        resolved_env: &'a ResolvedTypeEnv<'a>,
    ) -> (AccessContextSelection, &'a ContextFieldType) {
        if path_elements.len() == 2 {
            let context_type = resolved_env
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
                    Box::new(AccessContextSelection::Context(path_elements[0].clone())),
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
        let (_, column_path_elems, field_type) = path_elements[1..].iter().fold(
            (self_type_info, vec![], None),
            |(self_type_info, column_path_elems, _field_type), field_name| {
                let self_type_info =
                    self_type_info.expect("Type for the access selection is not defined");

                let (field_column_path, field_type) =
                    get_column(field_name, self_type_info, subsystem_entity_types);

                let field_composite_type = match base_type(
                    field_type,
                    &subsystem_primitive_types.values,
                    &subsystem_entity_types.values,
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
            ColumnIdPath {
                path: column_path_elems,
            },
            field_type.unwrap(),
        )
    } else {
        let (context_selection, context_field_type) = get_context(&path_elements, resolved_env);
        PathSelection::Context(context_selection, context_field_type)
    }
}
