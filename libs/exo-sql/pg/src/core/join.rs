// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Database;

use crate::pg_extension::PgExtension;
use crate::{ExpressionBuilder, SQLBuilder};

// Re-export the core LeftJoin type specialized to PgExtension
pub type LeftJoin = exo_sql_core::operation::LeftJoin<PgExtension>;

impl ExpressionBuilder for LeftJoin {
    /// Build expression of the form `<left> LEFT JOIN <right> ON <predicate>`.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        self.left().build(database, builder);
        builder.push_str(" LEFT JOIN ");
        self.right().build(database, builder);
        builder.push_str(" ON ");
        self.predicate().build(database, builder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_database_builder::*;
    use crate::{Column, SQLBuilder, predicate_ext::ConcretePredicate, table::Table};
    use exo_sql_core::SchemaObjectName;

    use multiplatform_test::multiplatform_test;

    #[multiplatform_test]
    fn basic_join() {
        let database = DatabaseBuilder::new()
            .table(
                "concerts",
                vec![pk("id"), fk("venue_id", "venues", "id", "venue_fk")],
            )
            .table("venues", vec![pk("id"), int("capacity")])
            .build();

        let concert_physical_table_id = database
            .get_table_id(&SchemaObjectName::new("concerts", None))
            .unwrap();
        let venue_physical_table_id = database
            .get_table_id(&SchemaObjectName::new("venues", None))
            .unwrap();

        let join_predicate = ConcretePredicate::Eq(
            Column::physical(
                database
                    .get_column_id(concert_physical_table_id, "venue_id")
                    .unwrap(),
                None,
            ),
            Column::physical(
                database
                    .get_column_id(venue_physical_table_id, "id")
                    .unwrap(),
                None,
            ),
        );

        let concert_table = Table::physical(concert_physical_table_id, None);
        let venue_table = Table::physical(venue_physical_table_id, None);
        let join = LeftJoin::new(concert_table, venue_table, join_predicate);

        let mut builder = SQLBuilder::new();
        join.build(&database, &mut builder);

        assert_binding!(
            builder.into_sql(),
            r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
        );
    }
}
