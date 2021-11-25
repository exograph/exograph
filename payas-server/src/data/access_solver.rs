use maybe_owned::MaybeOwned;
use payas_model::{
    model::{
        access::{AccessConextSelection, AccessExpression, AccessLogicalOp, AccessRelationalOp},
        system::ModelSystem,
    },
    sql::{column::Column, predicate::Predicate},
};
use serde_json::Value;

use std::ops::Not;

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
    system: &'a ModelSystem,
) -> ReducedExpression<'a> {
    match expr {
        AccessExpression::ContextSelection(selection) => {
            reduce_context_selection(selection, request_context)
                .map(|v| ReducedExpression::Value(v.to_owned()))
                .unwrap_or(ReducedExpression::UnresolvedContext(selection))
        }
        AccessExpression::Column(column_id) => {
            ReducedExpression::Column(system.create_column_with_id(column_id).into())
        }
        AccessExpression::StringLiteral(value) => {
            ReducedExpression::Value(Value::String(value.clone()))
        }
        AccessExpression::BooleanLiteral(value) => ReducedExpression::Value(Value::Bool(*value)),
        AccessExpression::NumberLiteral(value) => {
            ReducedExpression::Value(Value::Number((*value as i64).into()))
        }
        AccessExpression::LogicalOp(op) => {
            ReducedExpression::Predicate(reduce_logical_op(op, request_context, system))
        }
        AccessExpression::RelationalOp(op) => {
            ReducedExpression::Predicate(reduce_relational_op(op, request_context, system))
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
    system: &'a ModelSystem,
) -> Predicate<'a> {
    let (left, right) = op.sides();
    let left = reduce_expression(left, request_context, system);
    let right = reduce_expression(right, request_context, system);

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
    system: &'a ModelSystem,
) -> Predicate<'a> {
    match op {
        AccessLogicalOp::Not(underlying) => {
            let underlying = reduce_expression(underlying, request_context, system);
            match underlying {
                ReducedExpression::Value(_) => todo!(),
                ReducedExpression::Column(_) => todo!(),
                ReducedExpression::UnresolvedContext(_) => todo!(),
                ReducedExpression::Predicate(predicate) => predicate.not(),
            }
        }
        AccessLogicalOp::And(left, right) => {
            let left_predicate = match reduce_expression(left, request_context, system) {
                ReducedExpression::Predicate(predicate) => predicate,
                _ => panic!("Operand of 'And' isn't a predicate"),
            };

            let right_predicate = match reduce_expression(right, request_context, system) {
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
            let left_predicate = match reduce_expression(left, request_context, system) {
                ReducedExpression::Predicate(predicate) => predicate,
                _ => panic!("Operand of 'And' isn't a predicate"),
            };
            let right_predicate = match reduce_expression(right, request_context, system) {
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
    system: &'a ModelSystem,
) -> Predicate<'a> {
    match access_expression {
        AccessExpression::ContextSelection(_) => todo!(),
        AccessExpression::Column(_) => todo!(),
        AccessExpression::LogicalOp(op) => reduce_logical_op(op, request_context, system),
        AccessExpression::RelationalOp(op) => reduce_relational_op(op, request_context, system),
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
    use payas_model::{
        model::{column_id::ColumnId, system::ModelSystem},
        sql::{
            column::{IntBits, PhysicalColumn, PhysicalColumnType},
            PhysicalTable,
        },
    };
    use serde_json::json;
    use typed_generational_arena::{IgnoreGeneration, Index};

    use super::*;

    struct TestSystem {
        system: ModelSystem,
        table_id: Index<PhysicalTable, usize, IgnoreGeneration>,
        published_column_id: ColumnId,
        owner_id_column_id: ColumnId,
    }

    fn test_system() -> TestSystem {
        let published_column = PhysicalColumn {
            table_name: "article".to_string(),
            column_name: "published".to_string(),
            typ: PhysicalColumnType::Boolean,
            is_pk: false,
            is_autoincrement: false,
            is_nullable: false,
        };

        let owner_id_column = PhysicalColumn {
            table_name: "article".to_string(),
            column_name: "owner_id".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_64 },
            is_pk: false,
            is_autoincrement: false,
            is_nullable: false,
        };

        let table = PhysicalTable {
            name: "article".to_string(),
            columns: vec![published_column, owner_id_column],
        };

        let mut system = ModelSystem::default();
        let table_id = system.tables.insert(table);

        let table = &system.tables[table_id];
        let published_column_id = ColumnId::new(table_id, table.column_index("published").unwrap());
        let owner_id_column_id = ColumnId::new(table_id, table.column_index("owner_id").unwrap());

        TestSystem {
            system,
            table_id,
            published_column_id,
            owner_id_column_id,
        }
    }

    #[test]
    fn context_only() {
        // Scenario: AuthContext.role == "ROLE_ADMIN"

        let system = ModelSystem::default();

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
        let reduced = reduce_access(&test_ae, &context, &system);
        assert_eq!(reduced, Predicate::True);

        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let reduced = reduce_access(&test_ae, &context, &system);
        assert_eq!(reduced, Predicate::False)
    }

    #[test]
    fn context_and_dynamic() {
        // Scenario: AuthContext.role == "ROLE_ADMIN" || self.published

        let TestSystem {
            system,
            table_id,
            published_column_id,
            ..
        } = test_system();

        let test_ae = {
            let admin_access = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(AccessExpression::ContextSelection(
                    AccessConextSelection::Select(
                        Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                        "role".to_string(),
                    ),
                )),
                Box::new(AccessExpression::StringLiteral("ROLE_ADMIN".to_owned())),
            ));
            let user_access = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(AccessExpression::Column(published_column_id)),
                Box::new(AccessExpression::BooleanLiteral(true)),
            ));

            AccessExpression::LogicalOp(AccessLogicalOp::Or(
                Box::new(admin_access),
                Box::new(user_access),
            ))
        };

        let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
        let reduced = reduce_access(&test_ae, &context, &system);
        assert_eq!(reduced, Predicate::True);

        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let reduced = reduce_access(&test_ae, &context, &system);
        let table = &system.tables[table_id];
        assert_eq!(
            reduced,
            Predicate::Eq(
                table.get_column("published").unwrap().into(),
                Column::Literal(Box::new(true)).into()
            )
        )
    }

    #[test]
    fn context_compared_with_dynamic() {
        // Scenario: AuthContext.user_id == self.owner_id

        let TestSystem {
            system,
            table_id,
            owner_id_column_id,
            ..
        } = test_system();

        let test_ae = AccessExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(AccessExpression::ContextSelection(
                AccessConextSelection::Select(
                    Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                    "user_id".to_string(),
                ),
            )),
            Box::new(AccessExpression::Column(owner_id_column_id)),
        ));

        let table = &system.tables[table_id];

        let context = json!({ "AccessContext": {"user_id": "1"} });
        let reduced = reduce_access(&test_ae, &context, &system);
        assert_eq!(
            reduced,
            Predicate::Eq(
                table.get_column("owner_id").unwrap().into(),
                Column::Literal(Box::new("1".to_string())).into(),
            )
        );

        let context = json!({ "AccessContext": {"user_id": "2"} });
        let reduced = reduce_access(&test_ae, &context, &system);
        assert_eq!(
            reduced,
            Predicate::Eq(
                table.get_column("owner_id").unwrap().into(),
                Column::Literal(Box::new("2".to_string())).into(),
            )
        )
    }

    #[test]
    fn varied_rule_for_roles() {
        // Scenaior: AuthContext.role == "ROLE_ADMIN" || (AuthContext.role == "ROLE_USER" && self.published == true)

        let TestSystem {
            system,
            table_id,
            published_column_id,
            ..
        } = test_system();

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
                Box::new(AccessExpression::Column(published_column_id)),
                Box::new(AccessExpression::BooleanLiteral(true)),
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

        let table = &system.tables[table_id];

        // For admins, allow access without any further restrictions
        let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
        let reduced = reduce_access(&test_ae, &context, &system);
        assert_eq!(reduced, Predicate::True);

        // For users, allow only if the article is published
        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let reduced = reduce_access(&test_ae, &context, &system);
        assert_eq!(
            reduced,
            Predicate::Eq(
                table.get_column("published").unwrap().into(),
                Column::Literal(Box::new(true)).into(),
            )
        );

        // For others, do not allow
        let context = json!({ "AccessContext": {"role": "ROLE_GUEST"} });
        let reduced = reduce_access(&test_ae, &context, &system);
        assert_eq!(reduced, Predicate::False);
    }
}
