// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{ColumnId, Database};

use super::{ExpressionBuilder, SQLBuilder};
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Ordering {
    Asc,
    Desc,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OrderByElement(pub ColumnId, pub Ordering, pub Option<String>);

#[derive(Debug, PartialEq, Eq)]
pub struct OrderBy(pub Vec<OrderByElement>);

impl OrderByElement {
    pub fn new(column_id: ColumnId, ordering: Ordering, table_alias: Option<String>) -> Self {
        Self(column_id, ordering, table_alias)
    }
}

impl ExpressionBuilder for OrderByElement {
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        let column = self.0.get_column(database);
        match self.2 {
            Some(ref table_alias) => {
                builder.push_column(table_alias, &column.name);
            }
            None => {
                column.build(database, builder);
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::schema::test_helper::{int_column, pk_column, string_column};
    use crate::schema::{database_spec::DatabaseSpec, table_spec::TableSpec};

    #[test]
    fn single() {
        let database = DatabaseSpec::new(vec![TableSpec::new(
            "people",
            vec![pk_column("id"), int_column("age")],
        )])
        .to_database();

        let people_table_id = database.get_table_id("people").unwrap();

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
            "people",
            vec![pk_column("id"), string_column("name"), int_column("age")],
        )])
        .to_database();

        let table_id = database.get_table_id("people").unwrap();

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
