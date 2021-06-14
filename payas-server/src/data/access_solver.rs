use payas_model::sql::{column::Column, predicate::Predicate};
use serde_json::Value;

use super::operation_context::OperationContext;

#[derive(Debug)]
pub enum AccessExpression<'a> {
    ContextSelection(AccessConextSelection), // AuthContext.role
    Column(&'a Column<'a>), // self.id (special case of a boolean column such as self.published will be expanded to self.published == true when building an AccessExpression)
    StringLiteral(String),  // "ROLE_ADMIN"
    LogicalOp(AccessLogicalOp<'a>),
    RelationalOp(AccessRelationalOp<'a>),
}

#[derive(Debug)]
enum ReducedExpression<'a> {
    Value(Option<&'a Value>),
    Column(&'a Column<'a>),
    Predicate(&'a Predicate<'a>),
}

impl<'a> AccessExpression<'a> {
    fn reduce(
        &self,
        context: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> ReducedExpression<'a> {
        match self {
            AccessExpression::ContextSelection(selection) => {
                ReducedExpression::Column(literal_column(
                    selection.reduce(context).unwrap(),
                    operation_context,
                ))
                //ReducedExpression::Value(selection.reduce(context))
            }
            AccessExpression::Column(column) => ReducedExpression::Column(column),
            AccessExpression::StringLiteral(string) => ReducedExpression::Column(
                operation_context.create_column(Column::Literal(Box::new(string.clone()))),
            ),
            AccessExpression::LogicalOp(op) => {
                ReducedExpression::Predicate(op.reduce(context, operation_context))
            }
            AccessExpression::RelationalOp(op) => {
                ReducedExpression::Predicate(op.reduce(context, operation_context))
            }
        }
    }
}

impl AccessConextSelection {
    fn reduce<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        match self {
            AccessConextSelection::Single(key) => value.get(key),
            AccessConextSelection::Select(path, key) => {
                path.reduce(value).and_then(|value| value.get(key))
            }
        }
    }
}

#[derive(Debug)]
pub enum AccessConextSelection {
    Single(String),
    Select(Box<AccessConextSelection>, String),
}

#[derive(Debug)]
pub enum AccessRelationalOp<'a> {
    Eq(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    Neq(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    // Lt(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    // Lte(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    // Gt(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    // Gte(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
}

fn literal_column<'a>(
    value: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> &'a Column<'a> {
    let col = match value {
        Value::Null => todo!(),
        Value::Bool(v) => Column::Literal(Box::new(*v)),
        Value::Number(v) => Column::Literal(Box::new(v.as_i64().unwrap())), // Deal with the exact number type
        Value::String(v) => Column::Literal(Box::new(v.clone())),
        Value::Array(_) => todo!(),
        Value::Object(_) => todo!(),
    };

    operation_context.create_column(col)
}

impl<'a> AccessRelationalOp<'a> {
    fn reduce(
        &self,
        context: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> &'a Predicate<'a> {
        match self {
            AccessRelationalOp::Eq(left, right) => {
                let left = left.reduce(context, operation_context);
                let right = right.reduce(context, operation_context);

                match (left, right) {
                    (ReducedExpression::Column(left_col), ReducedExpression::Column(right_col)) => {
                        if left_col == right_col {
                            &Predicate::True
                        } else {
                            match (left_col, right_col) {
                                (Column::Literal(v1), Column::Literal(v2)) if v1 != v2 => {
                                    &Predicate::False
                                }
                                _ => operation_context
                                    .create_predicate(Predicate::Eq(left_col, right_col)),
                            }
                        }
                    }
                    (
                        ReducedExpression::Value(left_value),
                        ReducedExpression::Value(right_value),
                    ) => {
                        if left_value == right_value {
                            &Predicate::True
                        } else {
                            &Predicate::False
                        }
                    }
                    (ReducedExpression::Value(value), ReducedExpression::Column(column))
                    | (ReducedExpression::Column(column), ReducedExpression::Value(value)) => {
                        let value = literal_column(value.unwrap(), operation_context);

                        operation_context.create_predicate(Predicate::Eq(column, value))
                    }

                    _ => panic!("Operand of relational operator cannot be a predicate"),
                }
            }
            AccessRelationalOp::Neq(_, _) => todo!(),
        }
    }
}

#[derive(Debug)]
pub enum AccessLogicalOp<'a> {
    Not(Box<AccessExpression<'a>>),
    And(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
    Or(Box<AccessExpression<'a>>, Box<AccessExpression<'a>>),
}

impl<'a> AccessLogicalOp<'a> {
    fn reduce(
        &self,
        context: &'a Value,
        operation_context: &'a OperationContext<'a>,
    ) -> &'a Predicate<'a> {
        match self {
            AccessLogicalOp::Not(underlying) => {
                let underlying = underlying.reduce(context, operation_context);
                match underlying {
                    ReducedExpression::Value(_) => todo!(),
                    ReducedExpression::Column(_) => todo!(),
                    ReducedExpression::Predicate(predicate) => {
                        operation_context.create_predicate(predicate.not())
                    }
                }
            }
            AccessLogicalOp::And(left, right) => {
                let left_predicate = match left.reduce(context, operation_context) {
                    ReducedExpression::Predicate(predicate) => predicate,
                    _ => panic!("Operand of 'And' isn't a predicate"),
                };

                let right_predicate = match right.reduce(context, operation_context) {
                    ReducedExpression::Predicate(predicate) => predicate,
                    _ => panic!("Operand of 'And' isn't a predicate"),
                };

                match (left_predicate, right_predicate) {
                    (Predicate::False, _) => &Predicate::False,
                    (_, Predicate::False) => &Predicate::False,
                    (Predicate::True, Predicate::True) => &Predicate::True,
                    (Predicate::True, right_predicate) => right_predicate,
                    (left_predicate, Predicate::True) => left_predicate,
                    _ => operation_context.create_predicate(Predicate::And(
                        Box::new(left_predicate.clone()),
                        Box::new(right_predicate.clone()),
                    )),
                }
            }
            AccessLogicalOp::Or(left, right) => {
                let right_predicate = match right.reduce(context, operation_context) {
                    ReducedExpression::Predicate(predicate) => predicate,
                    _ => panic!("Operand of 'And' isn't a predicate"),
                };
                let left_predicate = match left.reduce(context, operation_context) {
                    ReducedExpression::Predicate(predicate) => predicate,
                    _ => panic!("Operand of 'And' isn't a predicate"),
                };

                match (left_predicate, right_predicate) {
                    (Predicate::True, _) => &Predicate::True,
                    (_, Predicate::True) => &Predicate::True,
                    (Predicate::False, Predicate::False) => &Predicate::False,

                    (Predicate::False, right_predicate) => right_predicate,
                    (left_predicate, Predicate::False) => left_predicate,
                    _ => operation_context.create_predicate(Predicate::And(
                        Box::new(left_predicate.clone()),
                        Box::new(right_predicate.clone()),
                    )),
                }
            }
        }
    }
}

fn reduce_access<'a>(
    access_expression: &'a AccessExpression<'a>,
    context: &'a Value,
    operation_context: &'a OperationContext<'a>,
) -> &'a Predicate<'a> {
    match access_expression {
        AccessExpression::ContextSelection(_) => todo!(),
        AccessExpression::Column(_) => todo!(),
        AccessExpression::LogicalOp(op) => op.reduce(context, operation_context),
        AccessExpression::RelationalOp(op) => op.reduce(context, operation_context),
        AccessExpression::StringLiteral(_) => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use payas_model::sql::column::{PhysicalColumn, PhysicalColumnType};
    use serde_json::json;

    use crate::execution::query_context::QueryContext;

    use super::*;

    #[test]
    fn context_only() {
        // Scenario: AuthContext.role == "ROLE_ADMIN"

        // SAFETY: Temporory code until we improve the design of OperationContext
        // For now, we don't acces query_context, so safe to use a null pointer
        let operation_context = unsafe {
            let null_query_context: *const QueryContext = ptr::null();
            let query_context: &QueryContext = &*null_query_context;
            OperationContext::new(&query_context)
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
        let reduced = reduce_access(&test_ae, &context, &operation_context);
        assert_eq!(reduced, &Predicate::True);

        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let reduced = reduce_access(&test_ae, &context, &operation_context);
        assert_eq!(reduced, &Predicate::False)
    }

    #[test]
    fn context_and_dynamic() {
        // Scenario: AuthContext.role == "ROLE_ADMIN" || self.published

        // SAFETY: Temporory code until we improve the design of OperationContext
        // For now, we don't acces query_context, so safe to use a null pointer
        let operation_context = unsafe {
            let null_query_context: *const QueryContext = ptr::null();
            let query_context: &QueryContext = &*null_query_context;
            OperationContext::new(&query_context)
        };

        let admin_access = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(AccessExpression::ContextSelection(
                AccessConextSelection::Select(
                    Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                    "role".to_string(),
                ),
            )),
            Box::new(AccessExpression::StringLiteral("ROLE_ADMIN".to_owned())),
        ));

        let published_column = PhysicalColumn {
            table_name: "article".to_string(),
            column_name: "published".to_string(),
            typ: PhysicalColumnType::Boolean,
            is_pk: false,
            is_autoincrement: false,
            references: None,
        };

        let user_access = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(AccessExpression::Column(
                operation_context.create_column(Column::Physical(&published_column)),
            )),
            Box::new(AccessExpression::Column(
                operation_context.create_column(Column::Literal(Box::new(true))),
            )),
        ));

        let test_ae = AccessExpression::LogicalOp(AccessLogicalOp::Or(
            Box::new(admin_access),
            Box::new(user_access),
        ));

        let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
        let reduced = reduce_access(&test_ae, &context, &operation_context);
        assert_eq!(reduced, &Predicate::True);

        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let reduced = reduce_access(&test_ae, &context, &operation_context);
        assert_eq!(
            reduced,
            &Predicate::Eq(
                &Column::Physical(&published_column),
                &Column::Literal(Box::new(true))
            )
        )
    }

    #[test]
    fn context_compared_with_dynamic() {
        // Scenario: AuthContext.user_id == self.owner_id

        // SAFETY: Temporory code until we improve the design of OperationContext
        // For now, we don't acces query_context, so safe to use a null pointer
        let operation_context = unsafe {
            let null_query_context: *const QueryContext = ptr::null();
            let query_context: &QueryContext = &*null_query_context;
            OperationContext::new(&query_context)
        };

        let owner_id_column = PhysicalColumn {
            table_name: "article".to_string(),
            column_name: "owner_id".to_string(),
            typ: PhysicalColumnType::String,
            is_pk: false,
            is_autoincrement: false,
            references: None,
        };

        let test_ae = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(AccessExpression::ContextSelection(
                AccessConextSelection::Select(
                    Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                    "user_id".to_string(),
                ),
            )),
            Box::new(AccessExpression::Column(
                operation_context.create_column(Column::Physical(&owner_id_column)),
            )),
        ));

        let context = json!({ "AccessContext": {"user_id": "1"} });
        let reduced = reduce_access(&test_ae, &context, &operation_context);
        assert_eq!(
            reduced,
            &Predicate::Eq(
                &Column::Literal(Box::new("1".to_string())),
                &Column::Physical(&owner_id_column),
            )
        );

        let context = json!({ "AccessContext": {"user_id": "2"} });
        let reduced = reduce_access(&test_ae, &context, &operation_context);

        assert_eq!(
            reduced,
            &Predicate::Eq(
                &Column::Literal(Box::new("2".to_string())),
                &Column::Physical(&owner_id_column),
            )
        )
    }

    #[test]
    fn varied_rule_for_roles() {
        // Scenaior: AuthContext.role == "ROLE_ADMIN" || (AuthContext.role == "ROLE_USER" && self.published == true)

        // SAFETY: Temporory code until we improve the design of OperationContext
        // For now, we don't acces query_context, so safe to use a null pointer
        let operation_context = unsafe {
            let null_query_context: *const QueryContext = ptr::null();
            let query_context: &QueryContext = &*null_query_context;
            OperationContext::new(&query_context)
        };

        let published_column = PhysicalColumn {
            table_name: "article".to_string(),
            column_name: "published".to_string(),
            typ: PhysicalColumnType::Boolean,
            is_pk: false,
            is_autoincrement: false,
            references: None,
        };

        let admin_access = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(AccessExpression::ContextSelection(
                AccessConextSelection::Select(
                    Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                    "role".to_string(),
                ),
            )),
            Box::new(AccessExpression::StringLiteral("ROLE_ADMIN".to_owned())),
        ));

        let user_access = {
            let role_rule = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(AccessExpression::ContextSelection(
                    AccessConextSelection::Select(
                        Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                        "role".to_string(),
                    ),
                )),
                Box::new(AccessExpression::StringLiteral("ROLE_USER".to_owned())),
            ));

            let data_rule = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(AccessExpression::Column(
                    operation_context.create_column(Column::Physical(&published_column)),
                )),
                Box::new(AccessExpression::Column(
                    operation_context.create_column(Column::Literal(Box::new(true))),
                )),
            ));

            AccessExpression::LogicalOp(AccessLogicalOp::And(
                Box::new(role_rule),
                Box::new(data_rule),
            ))
        };

        let test_ae = AccessExpression::LogicalOp(AccessLogicalOp::Or(
            Box::new(admin_access),
            Box::new(user_access),
        ));

        // For admins, allow access without any further restrictions
        let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
        let reduced = reduce_access(&test_ae, &context, &operation_context);
        assert_eq!(reduced, &Predicate::True);

        // For users, allow only if the article is published
        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let reduced = reduce_access(&test_ae, &context, &operation_context);
        assert_eq!(
            reduced,
            &Predicate::Eq(
                &Column::Physical(&published_column),
                &Column::Literal(Box::new(true)),
            )
        );

        // For others, do not allow
        let context = json!({ "AccessContext": {"role": "ROLE_GUEST"} });
        let reduced = reduce_access(&test_ae, &context, &operation_context);
        assert_eq!(reduced, &Predicate::False);
    }
}
