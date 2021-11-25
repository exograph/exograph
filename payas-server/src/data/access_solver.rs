use maybe_owned::MaybeOwned;
use payas_model::{
    model::access::{AccessConextSelection, AccessExpression, AccessLogicalOp, AccessRelationalOp},
    sql::{column::Column, predicate::Predicate},
};
use serde_json::Value;

use std::ops::Not;

use crate::execution::query_context::QueryContext;

#[derive(Debug)]
enum ReducedExpression<'a> {
    Value(Value),
    Column(MaybeOwned<'a, Column<'a>>),
    Predicate(Predicate<'a>),
    UnresolvedContext(&'a AccessConextSelection), // For example, AuthContext.role for an anonymous user
}

fn reduce_expression<'a>(
    expr: &'a AccessExpression,
    request_context: &'a Value,
    query_context: &'a QueryContext<'a>,
) -> ReducedExpression<'a> {
    match expr {
        AccessExpression::ContextSelection(selection) => {
            reduce_context_selection(selection, request_context)
                .map(|v| ReducedExpression::Value(v.to_owned()))
                .unwrap_or(ReducedExpression::UnresolvedContext(selection))
        }
        AccessExpression::Column(column_id) => {
            ReducedExpression::Column(query_context.create_column_with_id(column_id).into())
        }
        AccessExpression::StringLiteral(value) => {
            ReducedExpression::Value(Value::String(value.clone()))
        }
        AccessExpression::BooleanLiteral(value) => ReducedExpression::Value(Value::Bool(*value)),
        AccessExpression::NumberLiteral(value) => {
            ReducedExpression::Value(Value::Number((*value as i64).into()))
        }
        AccessExpression::LogicalOp(op) => {
            ReducedExpression::Predicate(reduce_logical_op(op, request_context, query_context))
        }
        AccessExpression::RelationalOp(op) => {
            ReducedExpression::Predicate(reduce_relational_op(op, request_context, query_context))
        }
    }
}

fn reduce_context_selection<'a>(
    context_selection: &AccessConextSelection,
    value: &'a Value,
) -> Option<&'a Value> {
    match context_selection {
        AccessConextSelection::Single(key) => value.get(key),
        AccessConextSelection::Select(path, key) => {
            reduce_context_selection(path, value).and_then(|value| value.get(key))
        }
    }
}

fn literal_column(value: Value) -> MaybeOwned<'static, Column<'static>> {
    match value {
        Value::Null => Column::Null,
        Value::Bool(v) => Column::Literal(Box::new(v)),
        Value::Number(v) => Column::Literal(Box::new(v.as_i64().unwrap())), // Deal with the exact number type
        Value::String(v) => Column::Literal(Box::new(v)),
        Value::Array(values) => Column::Literal(Box::new(values)),
        Value::Object(_) => todo!(),
    }
    .into()
}

fn reduce_relational_op<'a>(
    op: &'a AccessRelationalOp,
    request_context: &'a Value,
    query_context: &'a QueryContext<'a>,
) -> Predicate<'a> {
    let (left, right) = op.sides();
    let left = reduce_expression(left, request_context, query_context);
    let right = reduce_expression(right, request_context, query_context);

    match op {
        AccessRelationalOp::Eq(..) => match (left, right) {
            (ReducedExpression::UnresolvedContext(_), _)
            | (_, ReducedExpression::UnresolvedContext(_)) => Predicate::False,
            (ReducedExpression::Column(left_col), ReducedExpression::Column(right_col)) => {
                Predicate::eq(left_col, right_col)
            }
            (ReducedExpression::Value(left_value), ReducedExpression::Value(right_value)) => {
                (left_value == right_value).into()
            }
            (ReducedExpression::Value(value), ReducedExpression::Column(column))
            | (ReducedExpression::Column(column), ReducedExpression::Value(value)) => {
                Predicate::Eq(column, literal_column(value))
            }
            _ => panic!("Operand of relational operator cannot be a predicate"),
        },
        AccessRelationalOp::Neq(_, _) => todo!(),
        AccessRelationalOp::In(..) => match (left, right) {
            (ReducedExpression::UnresolvedContext(_), _)
            | (_, ReducedExpression::UnresolvedContext(_)) => Predicate::False,
            (ReducedExpression::Column(left_col), ReducedExpression::Column(right_col)) => {
                Predicate::In(left_col, right_col)
            }
            (ReducedExpression::Value(left_value), ReducedExpression::Value(right_value)) => {
                match right_value {
                    Value::Array(values) => values.contains(&left_value).into(),
                    _ => panic!("The right side operand of IN operator must be an array"),
                }
            }
            (ReducedExpression::Value(value), ReducedExpression::Column(column))
            | (ReducedExpression::Column(column), ReducedExpression::Value(value)) => {
                Predicate::In(column, literal_column(value))
            }
            _ => panic!("Operand of relational operator cannot be a predicate"),
        },
    }
}

fn reduce_logical_op<'a>(
    op: &'a AccessLogicalOp,
    request_context: &'a Value,
    query_context: &'a QueryContext<'a>,
) -> Predicate<'a> {
    match op {
        AccessLogicalOp::Not(underlying) => {
            let underlying = reduce_expression(underlying, request_context, query_context);
            match underlying {
                ReducedExpression::Value(_) => todo!(),
                ReducedExpression::Column(_) => todo!(),
                ReducedExpression::UnresolvedContext(_) => todo!(),
                ReducedExpression::Predicate(predicate) => predicate.not(),
            }
        }
        AccessLogicalOp::And(left, right) => {
            let left_predicate = match reduce_expression(left, request_context, query_context) {
                ReducedExpression::Predicate(predicate) => predicate,
                _ => panic!("Operand of 'And' isn't a predicate"),
            };

            let right_predicate = match reduce_expression(right, request_context, query_context) {
                ReducedExpression::Predicate(predicate) => predicate,
                _ => panic!("Operand of 'And' isn't a predicate"),
            };

            match (left_predicate, right_predicate) {
                (Predicate::False, _) => Predicate::False,
                (_, Predicate::False) => Predicate::False,
                (Predicate::True, Predicate::True) => Predicate::True,
                (Predicate::True, right_predicate) => right_predicate,
                (left_predicate, Predicate::True) => left_predicate,
                (left_predicate, right_predicate) => {
                    Predicate::and(left_predicate, right_predicate)
                }
            }
        }
        AccessLogicalOp::Or(left, right) => {
            let left_predicate = match reduce_expression(left, request_context, query_context) {
                ReducedExpression::Predicate(predicate) => predicate,
                _ => panic!("Operand of 'And' isn't a predicate"),
            };
            let right_predicate = match reduce_expression(right, request_context, query_context) {
                ReducedExpression::Predicate(predicate) => predicate,
                _ => panic!("Operand of 'And' isn't a predicate"),
            };

            match (left_predicate, right_predicate) {
                (Predicate::True, _) => Predicate::True,
                (_, Predicate::True) => Predicate::True,
                (Predicate::False, Predicate::False) => Predicate::False,

                (Predicate::False, right_predicate) => right_predicate,
                (left_predicate, Predicate::False) => left_predicate,
                (left_predicate, right_predicate) => {
                    Predicate::and(left_predicate, right_predicate)
                }
            }
        }
    }
}

pub fn reduce_access<'a>(
    access_expression: &'a AccessExpression,
    request_context: &'a Value,
    query_context: &'a QueryContext<'a>,
) -> Predicate<'a> {
    match access_expression {
        AccessExpression::ContextSelection(_) => todo!(),
        AccessExpression::Column(_) => todo!(),
        AccessExpression::LogicalOp(op) => reduce_logical_op(op, request_context, query_context),
        AccessExpression::RelationalOp(op) => {
            reduce_relational_op(op, request_context, query_context)
        }
        AccessExpression::StringLiteral(_) => todo!(),
        AccessExpression::BooleanLiteral(value) => {
            if *value {
                Predicate::True
            } else {
                Predicate::False
            }
        }
        AccessExpression::NumberLiteral(_) => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use serde_json::json;

    use crate::execution::query_context::QueryContext;

    use super::*;

    #[test]
    fn context_only() {
        // Scenario: AuthContext.role == "ROLE_ADMIN"

        // SAFETY: Temporory code until we improve the design of OperationContext
        // For now, we don't acces query_context, so safe to use a null pointer
        let query_context = unsafe {
            let null_query_context: *const QueryContext = ptr::null();
            let query_context: &QueryContext = &*null_query_context;
            query_context
        };

        let test_ae = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(AccessExpression::ContextSelection(
                AccessConextSelection::Select(
                    Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                    "role".to_string(),
                ),
            )),
            Box::new(AccessExpression::StringLiteral("ROLE_ADMIN".to_owned())),
        ));

        let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
        let reduced = reduce_access(&test_ae, &context, query_context);
        assert_eq!(reduced, Predicate::True);

        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let reduced = reduce_access(&test_ae, &context, query_context);
        assert_eq!(reduced, Predicate::False)
    }

    // TODO: Re-enable tests

    // #[test]
    // fn context_and_dynamic() {
    //     // Scenario: AuthContext.role == "ROLE_ADMIN" || self.published

    //     // SAFETY: Temporory code until we improve the design of OperationContext
    //     // For now, we don't acces query_context, so safe to use a null pointer
    //     let query_context = unsafe {
    //         let null_query_context: *const QueryContext = ptr::null();
    //         let query_context: &QueryContext = &*null_query_context;
    //         OperationContext::new(&query_context)
    //     };

    //     let admin_access = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
    //         Box::new(AccessExpression::ContextSelection(
    //             AccessConextSelection::Select(
    //                 Box::new(AccessConextSelection::Single("AccessContext".to_string())),
    //                 "role".to_string(),
    //             ),
    //         )),
    //         Box::new(AccessExpression::StringLiteral("ROLE_ADMIN".to_owned())),
    //     ));

    //     let published_column = PhysicalColumn {
    //         table_name: "article".to_string(),
    //         column_name: "published".to_string(),
    //         typ: PhysicalColumnType::Boolean,
    //         is_pk: false,
    //         is_autoincrement: false,
    //         references: None,
    //     };

    //     let user_access = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
    //         Box::new(AccessExpression::Column(
    //             query_context.create_column(Column::Physical(&published_column)),
    //         )),
    //         Box::new(AccessExpression::Column(
    //             query_context.create_column(Column::Literal(Box::new(true))),
    //         )),
    //     ));

    //     let test_ae = AccessExpression::LogicalOp(AccessLogicalOp::Or(
    //         Box::new(admin_access),
    //         Box::new(user_access),
    //     ));

    //     let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
    //     let reduced = reduce_access(&test_ae, &context, &query_context);
    //     assert_eq!(reduced, &Predicate::True);

    //     let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
    //     let reduced = reduce_access(&test_ae, &context, &query_context);
    //     assert_eq!(
    //         reduced,
    //         &Predicate::Eq(
    //             &Column::Physical(&published_column),
    //             &Column::Literal(Box::new(true))
    //         )
    //     )
    // }

    // #[test]
    // fn context_compared_with_dynamic() {
    //     // Scenario: AuthContext.user_id == self.owner_id

    //     // SAFETY: Temporory code until we improve the design of OperationContext
    //     // For now, we don't acces query_context, so safe to use a null pointer
    //     let query_context = unsafe {
    //         let null_query_context: *const QueryContext = ptr::null();
    //         let query_context: &QueryContext = &*null_query_context;
    //         OperationContext::new(&query_context)
    //     };

    //     let owner_id_column = PhysicalColumn {
    //         table_name: "article".to_string(),
    //         column_name: "owner_id".to_string(),
    //         typ: PhysicalColumnType::String,
    //         is_pk: false,
    //         is_autoincrement: false,
    //         references: None,
    //     };

    //     let test_ae = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
    //         Box::new(AccessExpression::ContextSelection(
    //             AccessConextSelection::Select(
    //                 Box::new(AccessConextSelection::Single("AccessContext".to_string())),
    //                 "user_id".to_string(),
    //             ),
    //         )),
    //         Box::new(AccessExpression::Column(
    //             query_context.create_column(Column::Physical(&owner_id_column)),
    //         )),
    //     ));

    //     let context = json!({ "AccessContext": {"user_id": "1"} });
    //     let reduced = reduce_access(&test_ae, &context, &query_context);
    //     assert_eq!(
    //         reduced,
    //         &Predicate::Eq(
    //             &Column::Literal(Box::new("1".to_string())),
    //             &Column::Physical(&owner_id_column),
    //         )
    //     );

    //     let context = json!({ "AccessContext": {"user_id": "2"} });
    //     let reduced = reduce_access(&test_ae, &context, &query_context);

    //     assert_eq!(
    //         reduced,
    //         &Predicate::Eq(
    //             &Column::Literal(Box::new("2".to_string())),
    //             &Column::Physical(&owner_id_column),
    //         )
    //     )
    // }

    // #[test]
    // fn varied_rule_for_roles() {
    //     // Scenaior: AuthContext.role == "ROLE_ADMIN" || (AuthContext.role == "ROLE_USER" && self.published == true)

    //     // SAFETY: Temporory code until we improve the design of OperationContext
    //     // For now, we don't acces query_context, so safe to use a null pointer
    //     let query_context = unsafe {
    //         let null_query_context: *const QueryContext = ptr::null();
    //         let query_context: &QueryContext = &*null_query_context;
    //         OperationContext::new(&query_context)
    //     };

    //     let published_column = PhysicalColumn {
    //         table_name: "article".to_string(),
    //         column_name: "published".to_string(),
    //         typ: PhysicalColumnType::Boolean,
    //         is_pk: false,
    //         is_autoincrement: false,
    //         references: None,
    //     };

    //     let admin_access = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
    //         Box::new(AccessExpression::ContextSelection(
    //             AccessConextSelection::Select(
    //                 Box::new(AccessConextSelection::Single("AccessContext".to_string())),
    //                 "role".to_string(),
    //             ),
    //         )),
    //         Box::new(AccessExpression::StringLiteral("ROLE_ADMIN".to_owned())),
    //     ));

    //     let user_access = {
    //         let role_rule = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
    //             Box::new(AccessExpression::ContextSelection(
    //                 AccessConextSelection::Select(
    //                     Box::new(AccessConextSelection::Single("AccessContext".to_string())),
    //                     "role".to_string(),
    //                 ),
    //             )),
    //             Box::new(AccessExpression::StringLiteral("ROLE_USER".to_owned())),
    //         ));

    //         let data_rule = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
    //             Box::new(AccessExpression::Column(
    //                 query_context.create_column(Column::Physical(&published_column)),
    //             )),
    //             Box::new(AccessExpression::Column(
    //                 query_context.create_column(Column::Literal(Box::new(true))),
    //             )),
    //         ));

    //         AccessExpression::LogicalOp(AccessLogicalOp::And(
    //             Box::new(role_rule),
    //             Box::new(data_rule),
    //         ))
    //     };

    //     let test_ae = AccessExpression::LogicalOp(AccessLogicalOp::Or(
    //         Box::new(admin_access),
    //         Box::new(user_access),
    //     ));

    //     // For admins, allow access without any further restrictions
    //     let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
    //     let reduced = reduce_access(&test_ae, &context, &query_context);
    //     assert_eq!(reduced, &Predicate::True);

    //     // For users, allow only if the article is published
    //     let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
    //     let reduced = reduce_access(&test_ae, &context, &query_context);
    //     assert_eq!(
    //         reduced,
    //         &Predicate::Eq(
    //             &Column::Physical(&published_column),
    //             &Column::Literal(Box::new(true)),
    //         )
    //     );

    //     // For others, do not allow
    //     let context = json!({ "AccessContext": {"role": "ROLE_GUEST"} });
    //     let reduced = reduce_access(&test_ae, &context, &query_context);
    //     assert_eq!(reduced, &Predicate::False);
    // }
}
