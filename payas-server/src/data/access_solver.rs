use maybe_owned::MaybeOwned;
use payas_model::{
    model::{
        access::{
            AccessConextSelection, AccessLogicalExpression, AccessPredicateExpression,
            AccessPrimitiveExpression, AccessRelationalOp,
        },
        system::ModelSystem,
    },
    sql::{column::Column, predicate::Predicate},
};
use serde_json::Value;

use std::ops::Not;

/// Solve access control logic.
/// The access control logic is expressed as a predicate expression. This method
/// tried to produce a simplest possible `Predicate` given the request context. It tries
/// to produce `Predicate::True` or `Predicate::False` when sufficient information is avaialble
/// to make such determination. This allows (in case of `Predicate::True`) to skip the database
/// filtering and (in case of `Predicate::False`) to return a "Not authorized" error (instead of an
/// empty/null result).
pub fn solve_access<'a>(
    expr: &'a AccessPredicateExpression,
    request_context: &'a Value,
    system: &'a ModelSystem,
) -> Predicate<'a> {
    solve_predicate_expression(expr, request_context, system)
}

fn solve_predicate_expression<'a>(
    expr: &'a AccessPredicateExpression,
    request_context: &'a Value,
    system: &'a ModelSystem,
) -> Predicate<'a> {
    match expr {
        AccessPredicateExpression::LogicalOp(op) => solve_logical_op(op, request_context, system),
        AccessPredicateExpression::RelationalOp(op) => {
            solve_relational_op(op, request_context, system)
        }
        AccessPredicateExpression::BooleanLiteral(value) => (*value).into(),
        AccessPredicateExpression::BooleanColumn(column_id) => {
            // Special case we have a boolean literal column by itself, so we create an equivalent Predicate::Eq.
            // This allows supporting expressions such as `self.published` (and not require `self.published = true`)
            Predicate::Eq(
                system.create_column_with_id(column_id).into(),
                Column::Literal(Box::new(true)).into(),
            )
        }
        AccessPredicateExpression::BooleanContextSelection(selection) => {
            let context_value = solve_context_selection(selection, request_context);
            context_value
                .map(|value| {
                    match value {
                        Value::Bool(value) => *value,
                        _ => unreachable!("Context selection must be a boolean"), // access_utils ensures that only boolean values are allowed
                    }
                })
                .unwrap_or(false) // context value wasn't found, so treat it as false
                .into()
        }
    }
}

fn solve_context_selection<'a>(
    context_selection: &AccessConextSelection,
    value: &'a Value,
) -> Option<&'a Value> {
    match context_selection {
        AccessConextSelection::Single(key) => value.get(key),
        AccessConextSelection::Select(path, key) => {
            solve_context_selection(path, value).and_then(|value| value.get(key))
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

fn solve_relational_op<'a>(
    op: &'a AccessRelationalOp,
    request_context: &'a Value,
    system: &'a ModelSystem,
) -> Predicate<'a> {
    #[derive(Debug)]
    enum SolvedPrimitiveExpression<'a> {
        Value(Value),
        Column(MaybeOwned<'a, Column<'a>>),
        UnresolvedContext(&'a AccessConextSelection), // For example, AuthContext.role for an anonymous user
    }

    fn reduce_primitive_expression<'a>(
        expr: &'a AccessPrimitiveExpression,
        request_context: &'a Value,
        system: &'a ModelSystem,
    ) -> SolvedPrimitiveExpression<'a> {
        match expr {
            AccessPrimitiveExpression::ContextSelection(selection) => {
                solve_context_selection(selection, request_context)
                    .map(|v| SolvedPrimitiveExpression::Value(v.to_owned()))
                    .unwrap_or(SolvedPrimitiveExpression::UnresolvedContext(selection))
            }
            AccessPrimitiveExpression::Column(column_id) => {
                SolvedPrimitiveExpression::Column(system.create_column_with_id(column_id).into())
            }
            AccessPrimitiveExpression::StringLiteral(value) => {
                SolvedPrimitiveExpression::Value(Value::String(value.clone()))
            }
            AccessPrimitiveExpression::BooleanLiteral(value) => {
                SolvedPrimitiveExpression::Value(Value::Bool(*value))
            }
            AccessPrimitiveExpression::NumberLiteral(value) => {
                SolvedPrimitiveExpression::Value(Value::Number((*value).into()))
            }
        }
    }

    let (left, right) = op.sides();
    let left = reduce_primitive_expression(left, request_context, system);
    let right = reduce_primitive_expression(right, request_context, system);

    let helper = |unresolved_context_predicate: Predicate<'static>,
                  column_predicate: fn(
        MaybeOwned<'a, Column<'a>>,
        MaybeOwned<'a, Column<'a>>,
    ) -> Predicate<'a>,
                  value_predicate: fn(Value, Value) -> Predicate<'a>|
     -> Predicate<'a> {
        match (left, right) {
            (SolvedPrimitiveExpression::UnresolvedContext(_), _)
            | (_, SolvedPrimitiveExpression::UnresolvedContext(_)) => unresolved_context_predicate,
            (
                SolvedPrimitiveExpression::Column(left_col),
                SolvedPrimitiveExpression::Column(right_col),
            ) => column_predicate(left_col, right_col),
            (
                SolvedPrimitiveExpression::Value(left_value),
                SolvedPrimitiveExpression::Value(right_value),
            ) => value_predicate(left_value, right_value),
            (
                SolvedPrimitiveExpression::Value(value),
                SolvedPrimitiveExpression::Column(column),
            )
            | (
                SolvedPrimitiveExpression::Column(column),
                SolvedPrimitiveExpression::Value(value),
            ) => column_predicate(column, literal_column(value)),
        }
    };

    match op {
        AccessRelationalOp::Eq(..) => helper(Predicate::False, Predicate::eq, |val1, val2| {
            (val1 == val2).into()
        }),
        AccessRelationalOp::Neq(_, _) => helper(
            Predicate::True, // If a context is undefined, declare the expression as a match. For example, `AuthContext.role != "ADMIN"` for anonymous user evaluates to true
            Predicate::neq,
            |val1, val2| (val1 != val2).into(),
        ),
        // For the next four, we could better optimize cases where values are known, but for now, we generate a predicate and let database handle it
        AccessRelationalOp::Lt(_, _) => helper(Predicate::False, Predicate::Lt, |val1, val2| {
            Predicate::Lt(literal_column(val1), literal_column(val2))
        }),
        AccessRelationalOp::Lte(_, _) => helper(Predicate::False, Predicate::Lte, |val1, val2| {
            Predicate::Lte(literal_column(val1), literal_column(val2))
        }),
        AccessRelationalOp::Gt(_, _) => helper(Predicate::False, Predicate::Gt, |val1, val2| {
            Predicate::Gt(literal_column(val1), literal_column(val2))
        }),
        AccessRelationalOp::Gte(_, _) => helper(Predicate::False, Predicate::Gte, |val1, val2| {
            Predicate::Gte(literal_column(val1), literal_column(val2))
        }),
        AccessRelationalOp::In(..) => helper(
            Predicate::False,
            Predicate::In,
            |left_value, right_value| match right_value {
                Value::Array(values) => values.contains(&left_value).into(),
                _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
            },
        ),
    }
}

fn solve_logical_op<'a>(
    op: &'a AccessLogicalExpression,
    request_context: &'a Value,
    system: &'a ModelSystem,
) -> Predicate<'a> {
    match op {
        AccessLogicalExpression::Not(underlying) => {
            let underlying = solve_predicate_expression(underlying, request_context, system);
            underlying.not()
        }
        AccessLogicalExpression::And(left, right) => {
            let left_predicate = solve_predicate_expression(left, request_context, system);
            let right_predicate = solve_predicate_expression(right, request_context, system);

            match (left_predicate, right_predicate) {
                (Predicate::False, _) | (_, Predicate::False) => Predicate::False,

                (Predicate::True, right_predicate) => right_predicate,
                (left_predicate, Predicate::True) => left_predicate,
                (left_predicate, right_predicate) => {
                    Predicate::and(left_predicate, right_predicate)
                }
            }
        }
        AccessLogicalExpression::Or(left, right) => {
            let left_predicate = solve_predicate_expression(left, request_context, system);
            let right_predicate = solve_predicate_expression(right, request_context, system);

            match (left_predicate, right_predicate) {
                (Predicate::True, _) | (_, Predicate::True) => Predicate::True,

                (Predicate::False, right_predicate) => right_predicate,
                (left_predicate, Predicate::False) => left_predicate,
                (left_predicate, right_predicate) => Predicate::or(left_predicate, right_predicate),
            }
        }
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
        dept1_id_column_id: ColumnId,
        dept2_id_column_id: ColumnId,
    }

    impl TestSystem {
        fn published_column(&self) -> MaybeOwned<Column> {
            let table = self.system.tables.get(self.table_id).unwrap();
            table.get_column("published").unwrap().into()
        }

        fn owner_id_column(&self) -> MaybeOwned<Column> {
            let table = self.system.tables.get(self.table_id).unwrap();
            table.get_column("owner_id").unwrap().into()
        }

        fn dept1_id_column(&self) -> MaybeOwned<Column> {
            let table = self.system.tables.get(self.table_id).unwrap();
            table.get_column("dept1_id").unwrap().into()
        }

        fn dept2_id_column(&self) -> MaybeOwned<Column> {
            let table = self.system.tables.get(self.table_id).unwrap();
            table.get_column("dept2_id").unwrap().into()
        }
    }

    fn test_system() -> TestSystem {
        fn mk_column(column_name: &str, typ: PhysicalColumnType) -> PhysicalColumn {
            PhysicalColumn {
                table_name: "article".to_string(),
                column_name: column_name.to_string(),
                typ,
                is_pk: false,
                is_autoincrement: false,
                is_nullable: false,
            }
        }

        let table = PhysicalTable {
            name: "article".to_string(),
            columns: vec![
                mk_column("published", PhysicalColumnType::Boolean),
                mk_column("owner_id", PhysicalColumnType::Int { bits: IntBits::_64 }),
                mk_column("dept1_id", PhysicalColumnType::Int { bits: IntBits::_64 }),
                mk_column("dept2_id", PhysicalColumnType::Int { bits: IntBits::_64 }),
            ],
        };

        let mut system = ModelSystem::default();
        let table_id = system.tables.insert(table);

        let table = &system.tables[table_id];
        let published_column_id = ColumnId::new(table_id, table.column_index("published").unwrap());
        let owner_id_column_id = ColumnId::new(table_id, table.column_index("owner_id").unwrap());
        let dept1_id_column_id = ColumnId::new(table_id, table.column_index("dept1_id").unwrap());
        let dept2_id_column_id = ColumnId::new(table_id, table.column_index("dept2_id").unwrap());

        TestSystem {
            system,
            table_id,
            published_column_id,
            owner_id_column_id,
            dept1_id_column_id,
            dept2_id_column_id,
        }
    }

    fn context_selection(head: &str, tail: &[&str]) -> AccessConextSelection {
        match tail {
            [] => AccessConextSelection::Single(head.to_string()),
            [init @ .., last] => AccessConextSelection::Select(
                Box::new(context_selection(head, init)),
                last.to_string(),
            ),
        }
    }

    fn context_selection_expr(head: &str, tail: &[&str]) -> Box<AccessPrimitiveExpression> {
        Box::new(AccessPrimitiveExpression::ContextSelection(
            context_selection(head, tail),
        ))
    }

    fn test_relational_op<'a>(
        test_system: &'a TestSystem,
        op: fn(
            Box<AccessPrimitiveExpression>,
            Box<AccessPrimitiveExpression>,
        ) -> AccessRelationalOp,
        context_match_predicate: fn(
            MaybeOwned<'a, Column<'a>>,
            MaybeOwned<'a, Column<'a>>,
        ) -> Predicate<'a>,
        context_mismatch_predicate: fn(
            MaybeOwned<'a, Column<'a>>,
            MaybeOwned<'a, Column<'a>>,
        ) -> Predicate<'a>,
        context_missing_predicate: Predicate<'a>,
        context_value_predicate: fn(
            MaybeOwned<'a, Column<'a>>,
            MaybeOwned<'a, Column<'a>>,
        ) -> Predicate<'a>,
        column_column_predicate: fn(
            MaybeOwned<'a, Column<'a>>,
            MaybeOwned<'a, Column<'a>>,
        ) -> Predicate<'a>,
    ) {
        let TestSystem {
            system,
            owner_id_column_id,
            dept1_id_column_id,
            dept2_id_column_id,
            ..
        } = &test_system;

        // Case 1: Both values from AuthContext
        {
            let test_ae = AccessPredicateExpression::RelationalOp(op(
                context_selection_expr("AccessContext", &["token1"]),
                context_selection_expr("AccessContext", &["token2"]),
            ));

            let context =
                json!({ "AccessContext": {"token1": "token_value", "token2": "token_value"} });
            let solved = solve_access(&test_ae, &context, system);
            assert_eq!(
                solved,
                context_match_predicate(
                    Column::Literal(Box::new("token_value".to_string())).into(),
                    Column::Literal(Box::new("token_value".to_string())).into(),
                )
            );

            // The mismatch case doesn't make sense for lt/lte/gt/gte, but since we don't optimize
            // (to reduce obvious matches such as 5 < 6 => Predicate::True/False) in those cases,
            // the unoptimized predicate created works for both match and mismatch cases.
            let context =
                json!({ "AccessContext": {"token1": "token_value1", "token2": "token_value2"} });
            let solved = solve_access(&test_ae, &context, system);
            assert_eq!(
                solved,
                context_mismatch_predicate(
                    Column::Literal(Box::new("token_value1".to_string())).into(),
                    Column::Literal(Box::new("token_value2".to_string())).into(),
                )
            );
        }

        // One value from AuthContext and other from a column
        {
            let test_context_column = |test_ae: AccessPredicateExpression| {
                let context = json!({ "AccessContext": {"user_id": "u1"} });
                let solved = solve_access(&test_ae, &context, system);
                assert_eq!(
                    solved,
                    context_value_predicate(
                        test_system.owner_id_column(),
                        Column::Literal(Box::new("u1".to_string())).into(),
                    )
                );

                let context = Value::Null; // No user_id, so we can definitely declare it Predicate::False
                let solved = solve_access(&test_ae, &context, system);
                assert_eq!(solved, context_missing_predicate);
            };

            // Once test with `context op column` and then `column op context`
            test_context_column(AccessPredicateExpression::RelationalOp(op(
                context_selection_expr("AccessContext", &["user_id"]),
                Box::new(AccessPrimitiveExpression::Column(
                    owner_id_column_id.clone(),
                )),
            )));

            test_context_column(AccessPredicateExpression::RelationalOp(op(
                Box::new(AccessPrimitiveExpression::Column(
                    owner_id_column_id.clone(),
                )),
                context_selection_expr("AccessContext", &["user_id"]),
            )));
        }

        // Both values from columns
        {
            let test_ae = AccessPredicateExpression::RelationalOp(op(
                Box::new(AccessPrimitiveExpression::Column(
                    dept1_id_column_id.clone(),
                )),
                Box::new(AccessPrimitiveExpression::Column(
                    dept2_id_column_id.clone(),
                )),
            ));

            let context = Value::Null; // context is irrelevant
            let solved = solve_access(&test_ae, &context, system);
            assert_eq!(
                solved,
                column_column_predicate(
                    test_system.dept1_id_column(),
                    test_system.dept2_id_column(),
                )
            );
        }
    }

    #[test]
    fn basic_eq() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Eq,
            |_, _| Predicate::True,
            |_, _| Predicate::False,
            Predicate::False,
            Predicate::Eq,
            Predicate::Eq,
        );
    }

    #[test]
    fn basic_neq() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Neq,
            |_, _| Predicate::False,
            |_, _| Predicate::True,
            Predicate::True,
            Predicate::Neq,
            Predicate::Neq,
        );
    }

    #[test]
    fn basic_lt() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Lt,
            Predicate::Lt,
            Predicate::Lt,
            Predicate::False,
            Predicate::Lt,
            Predicate::Lt,
        );
    }

    #[test]
    fn basic_lte() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Lte,
            Predicate::Lte,
            Predicate::Lte,
            Predicate::False,
            Predicate::Lte,
            Predicate::Lte,
        );
    }

    #[test]
    fn basic_gt() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Gt,
            Predicate::Gt,
            Predicate::Gt,
            Predicate::False,
            Predicate::Gt,
            Predicate::Gt,
        );
    }

    #[test]
    fn basic_gte() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Gte,
            Predicate::Gte,
            Predicate::Gte,
            Predicate::False,
            Predicate::Gte,
            Predicate::Gte,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn test_logical_op<'a>(
        test_system: &'a TestSystem,
        op: fn(
            Box<AccessPredicateExpression>,
            Box<AccessPredicateExpression>,
        ) -> AccessLogicalExpression,
        both_value_true: Predicate,
        both_value_false: Predicate,
        one_value_true: Predicate,
        one_literal_true_other_column: fn(Predicate<'a>) -> Predicate<'a>,
        one_literal_false_other_column: fn(Predicate<'a>) -> Predicate<'a>,
        both_columns: fn(
            Box<MaybeOwned<'a, Predicate<'a>>>,
            Box<MaybeOwned<'a, Predicate<'a>>>,
        ) -> Predicate<'a>,
    ) {
        let TestSystem {
            system,
            dept1_id_column_id,
            dept2_id_column_id,
            ..
        } = &test_system;

        {
            // Two literals
            let context = Value::Null; // context is irrelevant

            let scenarios = [
                (true, true, &both_value_true),
                (true, false, &one_value_true),
                (false, true, &one_value_true),
                (false, false, &both_value_false),
            ];

            for (l1, l2, expected) in scenarios.iter() {
                let test_ae = AccessPredicateExpression::LogicalOp(op(
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l1)),
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l2)),
                ));

                let solved = solve_access(&test_ae, &context, system);
                assert_eq!(&&solved, expected);
            }
        }
        {
            // Two context values
            let context = json!({ "AccessContext": {"v1": true, "v1_clone": true, "v2": false, "v2_clone": false} });

            let scenarios = [
                ("v1", "v1_clone", &both_value_true),
                ("v1", "v2", &one_value_true),
                ("v2", "v1", &one_value_true),
                ("v2", "v2_clone", &both_value_false),
            ];

            for (c1, c2, expected) in scenarios.iter() {
                let test_ae = AccessPredicateExpression::LogicalOp(op(
                    Box::new(AccessPredicateExpression::BooleanContextSelection(
                        AccessConextSelection::Select(
                            Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                            c1.to_string(),
                        ),
                    )),
                    Box::new(AccessPredicateExpression::BooleanContextSelection(
                        AccessConextSelection::Select(
                            Box::new(AccessConextSelection::Single("AccessContext".to_string())),
                            c2.to_string(),
                        ),
                    )),
                ));

                let solved = solve_access(&test_ae, &context, system);
                assert_eq!(&&solved, expected);
            }
        }
        {
            // One literal and other a column
            let scenarios = [
                (true, &one_literal_true_other_column),
                (false, &one_literal_false_other_column),
            ];
            let context = Value::Null; // context is irrelevant

            for (l, predicate_fn) in scenarios.iter() {
                let test_ae = AccessPredicateExpression::LogicalOp(op(
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l)),
                    Box::new(AccessPredicateExpression::BooleanColumn(
                        dept1_id_column_id.clone(),
                    )),
                ));

                let solved = solve_access(&test_ae, &context, system);
                assert_eq!(
                    solved,
                    predicate_fn(Predicate::Eq(
                        test_system.dept1_id_column(),
                        Column::Literal(Box::new(true)).into()
                    ))
                );

                // The swapped version
                let test_ae = AccessPredicateExpression::LogicalOp(op(
                    Box::new(AccessPredicateExpression::BooleanColumn(
                        dept1_id_column_id.clone(),
                    )),
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l)),
                ));

                let solved = solve_access(&test_ae, &context, system);
                assert_eq!(
                    solved,
                    predicate_fn(Predicate::Eq(
                        test_system.dept1_id_column(),
                        Column::Literal(Box::new(true)).into()
                    ))
                );
            }
        }

        {
            // Two columns
            let test_ae = AccessPredicateExpression::LogicalOp(op(
                Box::new(AccessPredicateExpression::BooleanColumn(
                    dept1_id_column_id.clone(),
                )),
                Box::new(AccessPredicateExpression::BooleanColumn(
                    dept2_id_column_id.clone(),
                )),
            ));

            let context = Value::Null; // context is irrelevant
            let solved = solve_access(&test_ae, &context, system);
            assert_eq!(
                solved,
                both_columns(
                    Box::new(
                        Predicate::Eq(
                            test_system.dept1_id_column(),
                            Column::Literal(Box::new(true)).into()
                        )
                        .into()
                    ),
                    Box::new(
                        Predicate::Eq(
                            test_system.dept2_id_column(),
                            Column::Literal(Box::new(true)).into()
                        )
                        .into()
                    )
                )
            )
        }
    }

    #[test]
    fn basic_and() {
        test_logical_op(
            &test_system(),
            AccessLogicalExpression::And,
            Predicate::True,
            Predicate::False,
            Predicate::False,
            |p| p,
            |_| Predicate::False,
            Predicate::And,
        );
    }

    #[test]
    fn basic_or() {
        test_logical_op(
            &test_system(),
            AccessLogicalExpression::Or,
            Predicate::True,
            Predicate::False,
            Predicate::True,
            |_| Predicate::True,
            |p| p,
            Predicate::Or,
        );
    }

    #[test]
    fn context_only() {
        // Scenario: AuthContext.role == "ROLE_ADMIN"

        let system = ModelSystem::default();

        let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", &["role"]),
            Box::new(AccessPrimitiveExpression::StringLiteral(
                "ROLE_ADMIN".to_owned(),
            )),
        ));

        let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
        let solved = solve_access(&test_ae, &context, &system);
        assert_eq!(solved, Predicate::True);

        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let solved = solve_access(&test_ae, &context, &system);
        assert_eq!(solved, Predicate::False)
    }

    #[test]
    fn context_and_dynamic() {
        // Scenario: AuthContext.role == "ROLE_ADMIN" || self.published

        let test_system = test_system();
        let TestSystem {
            system,
            published_column_id,
            ..
        } = &test_system;

        let test_ae = {
            let admin_access = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                context_selection_expr("AccessContext", &["role"]),
                Box::new(AccessPrimitiveExpression::StringLiteral(
                    "ROLE_ADMIN".to_owned(),
                )),
            ));
            let user_access = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(AccessPrimitiveExpression::Column(
                    published_column_id.clone(),
                )),
                Box::new(AccessPrimitiveExpression::BooleanLiteral(true)),
            ));

            AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Or(
                Box::new(admin_access),
                Box::new(user_access),
            ))
        };

        let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(solved, Predicate::True);

        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(
            solved,
            Predicate::Eq(
                test_system.published_column(),
                Column::Literal(Box::new(true)).into()
            )
        )
    }

    #[test]
    fn context_compared_with_dynamic() {
        // Scenario: AuthContext.user_id == self.owner_id

        let test_system = test_system();
        let TestSystem {
            system,
            owner_id_column_id,
            ..
        } = &test_system;

        let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", &["user_id"]),
            Box::new(AccessPrimitiveExpression::Column(
                owner_id_column_id.clone(),
            )),
        ));

        let context = json!({ "AccessContext": {"user_id": "1"} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(
            solved,
            Predicate::Eq(
                test_system.owner_id_column(),
                Column::Literal(Box::new("1".to_string())).into(),
            )
        );

        let context = json!({ "AccessContext": {"user_id": "2"} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(
            solved,
            Predicate::Eq(
                test_system.owner_id_column(),
                Column::Literal(Box::new("2".to_string())).into(),
            )
        )
    }

    #[test]
    fn varied_rule_for_roles() {
        // Scenaior: AuthContext.role == "ROLE_ADMIN" || (AuthContext.role == "ROLE_USER" && self.published == true)

        let test_system = test_system();
        let TestSystem {
            system,
            published_column_id,
            ..
        } = &test_system;

        let admin_access = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", &["role"]),
            Box::new(AccessPrimitiveExpression::StringLiteral(
                "ROLE_ADMIN".to_owned(),
            )),
        ));

        let user_access = {
            let role_rule = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                context_selection_expr("AccessContext", &["role"]),
                Box::new(AccessPrimitiveExpression::StringLiteral(
                    "ROLE_USER".to_owned(),
                )),
            ));

            let data_rule = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(AccessPrimitiveExpression::Column(
                    published_column_id.clone(),
                )),
                Box::new(AccessPrimitiveExpression::BooleanLiteral(true)),
            ));

            AccessPredicateExpression::LogicalOp(AccessLogicalExpression::And(
                Box::new(role_rule),
                Box::new(data_rule),
            ))
        };

        let test_ae = AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Or(
            Box::new(admin_access),
            Box::new(user_access),
        ));

        // For admins, allow access without any further restrictions
        let context = json!({ "AccessContext": {"role": "ROLE_ADMIN"} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(solved, Predicate::True);

        // For users, allow only if the article is published
        let context = json!({ "AccessContext": {"role": "ROLE_USER"} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(
            solved,
            Predicate::Eq(
                test_system.published_column(),
                Column::Literal(Box::new(true)).into(),
            )
        );

        // For other roles, do not allow
        let context = json!({ "AccessContext": {"role": "ROLE_GUEST"} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(solved, Predicate::False);

        // For anonymous users, too, do not allow (irrelevant context content that doesn't define a user role)
        let context = json!({ "Foo": "bar" });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(solved, Predicate::False);

        // For anonymous users, too, do not allow (no context content)
        let solved = solve_access(&test_ae, &Value::Null, system);
        assert_eq!(solved, Predicate::False);
    }

    #[test]
    fn top_level_boolean_literal() {
        // Scenario: true or false
        let system = ModelSystem::default();

        let test_ae = AccessPredicateExpression::BooleanLiteral(true);
        let context = Value::Null; // irrelevant context content
        let solved = solve_access(&test_ae, &context, &system);
        assert_eq!(solved, Predicate::True);

        let test_ae = AccessPredicateExpression::BooleanLiteral(false);
        let context = Value::Null; // irrelevant context content
        let solved = solve_access(&test_ae, &context, &system);
        assert_eq!(solved, Predicate::False);
    }

    #[test]
    fn top_level_boolean_column() {
        // Scenario: self.published

        let test_system = test_system();
        let TestSystem {
            system,
            published_column_id,
            ..
        } = &test_system;

        let test_ae = AccessPredicateExpression::BooleanColumn(published_column_id.clone());

        let context = Value::Null; // irrelevant context content
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(
            solved,
            Predicate::Eq(
                test_system.published_column(),
                Column::Literal(Box::new(true)).into()
            )
        )
    }

    #[test]
    fn top_level_boolean_context() {
        // Scenario: AuthComntext.is_admin

        let test_system = test_system();
        let TestSystem { system, .. } = &test_system;

        let test_ae = AccessPredicateExpression::BooleanContextSelection(context_selection(
            "AccessContext",
            &["is_admin"],
        ));

        let context = json!({ "AccessContext": {"is_admin": true} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(solved, Predicate::True);

        let context = json!({ "AccessContext": {"is_admin": false} });
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(solved, Predicate::False);

        let context = Value::Null; // context not provided, so we should assume that the user is not an admin
        let solved = solve_access(&test_ae, &context, system);
        assert_eq!(solved, Predicate::False)
    }
}
