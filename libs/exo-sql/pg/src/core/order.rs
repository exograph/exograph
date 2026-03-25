// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::{Database, Ordering};

use crate::core::pg_extension::{PgExtension, VectorDistanceOperand};
use crate::{ExpressionBuilder, SQLBuilder, core::vector::VectorDistance};

// Re-export the core OrderBy types specialized to PgExtension
pub type OrderBy = exo_sql_core::operation::OrderBy<PgExtension>;
pub type OrderByElement = exo_sql_core::operation::OrderByElement<PgExtension>;
pub type OrderByElementExpr = exo_sql_core::operation::OrderByElementExpr<PgExtension>;

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
            OrderByElementExpr::Extension(PgExtension::VectorDistance(lhs, rhs, function)) => {
                VectorDistance::new((lhs, self.2.as_ref()), (rhs, self.2.as_ref()), *function)
                    .build(database, builder);
            }
            // PgExtension is a flat enum shared across Column, Function, and OrderBy.
            // Only VectorDistance is valid here.
            OrderByElementExpr::Extension(_) => {
                unreachable!("Non-orderby PgExtension variant used in OrderByElementExpr")
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
    use std::vec;

    use super::*;
    use crate::test_database_builder::*;
    use exo_sql_core::{Ordering, SchemaObjectName};

    use multiplatform_test::multiplatform_test;

    #[multiplatform_test]
    fn single() {
        let database = DatabaseBuilder::new()
            .table("people", vec![pk("id"), int("age")])
            .build();

        let people_table_id = database
            .get_table_id(&SchemaObjectName::new("people", None))
            .unwrap();

        let age_col = database.get_column_id(people_table_id, "age").unwrap();

        let order_by = OrderBy::new(vec![OrderByElement::new(age_col, Ordering::Desc, None)]);

        assert_binding!(
            order_by.to_sql(&database),
            r#"ORDER BY "people"."age" DESC"#
        );
    }

    #[multiplatform_test]
    fn multiple() {
        let database = DatabaseBuilder::new()
            .table("people", vec![pk("id"), string("name"), int("age")])
            .build();

        let table_id = database
            .get_table_id(&SchemaObjectName::new("people", None))
            .unwrap();

        let name_col = database.get_column_id(table_id, "name").unwrap();
        let age_col = database.get_column_id(table_id, "age").unwrap();

        {
            let order_by = OrderBy::new(vec![
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
            let order_by = OrderBy::new(vec![
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
