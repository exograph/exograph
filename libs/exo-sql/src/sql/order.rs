// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::vector::VectorDistanceFunction;
use crate::{sql::vector::VectorDistance, ColumnId, Database, SQLParamContainer};

use super::{ExpressionBuilder, SQLBuilder};
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Ordering {
    Asc,
    Desc,
}

#[derive(Debug, PartialEq)]
pub struct OrderByElement(pub OrderByElementExpr, pub Ordering, pub Option<String>);

#[derive(Debug, PartialEq)]
pub enum VectorDistanceOperand {
    PhysicalColumn(ColumnId),
    Param(SQLParamContainer),
}

#[derive(Debug, PartialEq)]
pub enum OrderByElementExpr {
    Column(ColumnId),
    VectorDistance(
        VectorDistanceOperand,
        VectorDistanceOperand,
        VectorDistanceFunction,
    ),
}

#[derive(Debug, PartialEq)]
pub struct OrderBy(pub Vec<OrderByElement>);

impl OrderByElement {
    pub fn new(column_id: ColumnId, ordering: Ordering, table_alias: Option<String>) -> Self {
        Self(OrderByElementExpr::Column(column_id), ordering, table_alias)
    }
}

impl ExpressionBuilder for OrderByElement {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match &self.0 {
            OrderByElementExpr::Column(column_id) => {
                let column = column_id.get_column(database);
                match self.2 {
                    Some(ref table_alias) => {
                        builder.push_column_with_table_alias(&column.name, table_alias);
                    }
                    None => {
                        column.build(database, builder);
                    }
                }
            }
            OrderByElementExpr::VectorDistance(lhs, rhs, function) => {
                VectorDistance::new((lhs, self.2.as_ref()), (rhs, self.2.as_ref()), *function)
                    .build(database, builder);
            }
        }
        builder.push_space();

        if self.1 == Ordering::Asc {
            builder.push_str("ASC");
        } else {
            builder.push_str("DESC");
        }
    }
}

impl ExpressionBuilder for OrderBy {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        builder.push_str("ORDER BY ");
        builder.push_elems(database, &self.0, ", ");
    }
}

impl ExpressionBuilder for (&VectorDistanceOperand, Option<&String>) {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        match &self.0 {
            VectorDistanceOperand::PhysicalColumn(column_id) => {
                let column = column_id.get_column(database);
                match &self.1 {
                    Some(table_alias) => {
                        builder.push_column_with_table_alias(&column.name, table_alias);
                    }
                    None => {
                        column.build(database, builder);
                    }
                }
            }
            VectorDistanceOperand::Param(param) => {
                builder.push_param(param.param());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::schema::test_helper::{int_column, pk_column, string_column};
    use crate::schema::{database_spec::DatabaseSpec, table_spec::TableSpec};
    use crate::PhysicalTableName;

    #[test]
    fn single() {
        let database = DatabaseSpec::new(vec![TableSpec::new(
            PhysicalTableName::new("people", None),
            vec![pk_column("id"), int_column("age")],
            vec![],
        )])
        .to_database();

        let people_table_id = database
            .get_table_id(&PhysicalTableName::new("people", None))
            .unwrap();

        let age_col = database.get_column_id(people_table_id, "age").unwrap();

        let order_by = OrderBy(vec![OrderByElement::new(age_col, Ordering::Desc, None)]);

        assert_binding!(
            order_by.to_sql(&database),
            r#"ORDER BY "people"."age" DESC"#
        );
    }

    #[test]
    fn multiple() {
        let database = DatabaseSpec::new(vec![TableSpec::new(
            PhysicalTableName::new("people", None),
            vec![pk_column("id"), string_column("name"), int_column("age")],
            vec![],
        )])
        .to_database();

        let table_id = database
            .get_table_id(&PhysicalTableName::new("people", None))
            .unwrap();

        let name_col = database.get_column_id(table_id, "name").unwrap();
        let age_col = database.get_column_id(table_id, "age").unwrap();

        {
            let order_by = OrderBy(vec![
                OrderByElement::new(name_col, Ordering::Asc, None),
                OrderByElement::new(age_col, Ordering::Desc, None),
            ]);

            assert_binding!(
                order_by.to_sql(&database),
                r#"ORDER BY "people"."name" ASC, "people"."age" DESC"#
            );
        }

        // Reverse the order and it should be reflected in the statement
        {
            let order_by = OrderBy(vec![
                OrderByElement::new(age_col, Ordering::Desc, None),
                OrderByElement::new(name_col, Ordering::Asc, None),
            ]);

            assert_binding!(
                order_by.to_sql(&database),
                r#"ORDER BY "people"."age" DESC, "people"."name" ASC"#
            );
        }
    }
}
