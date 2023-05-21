// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::Database;

use super::{predicate::ConcretePredicate, table::Table, ExpressionBuilder, SQLBuilder};

/// Represents a join between two tables. Currently, supports only left join.
#[derive(Debug, PartialEq)]
pub struct LeftJoin {
    /// The left table in the join such as `concerts`.
    left: Box<Table>,
    /// The right table in the join such as `venues`.
    right: Box<Table>,
    /// The join predicate such as `concerts.venue_id = venues.id`.
    predicate: ConcretePredicate,
}

impl LeftJoin {
    pub fn new(left: Table, right: Table, predicate: ConcretePredicate) -> Self {
        LeftJoin {
            left: Box::new(left),
            right: Box::new(right),
            predicate,
        }
    }

    pub fn left(&self) -> &Table {
        &self.left
    }
}

impl ExpressionBuilder for LeftJoin {
    /// Build expression of the form `<left> LEFT JOIN <right> ON <predicate>`.
    fn build(&self, database: &Database, builder: &mut SQLBuilder) {
        self.left.build(database, builder);
        builder.push_str(" LEFT JOIN ");
        self.right.build(database, builder);
        builder.push_str(" ON ");
        self.predicate.build(database, builder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::test_helper::{int_column, pk_column, pk_reference_column};
    use crate::{
        schema::{database_spec::DatabaseSpec, table_spec::TableSpec},
        Column,
    };

    #[test]
    fn basic_join() {
        let database = DatabaseSpec::new(vec![
            TableSpec::new(
                "concerts",
                vec![pk_column("id"), pk_reference_column("venue_id", "venues")],
            ),
            TableSpec::new("venues", vec![pk_column("id"), int_column("capacity")]),
        ])
        .to_database();

        let concert_physical_table_id = database.get_table_id("concerts").unwrap();
        let venue_physical_table_id = database.get_table_id("venues").unwrap();

        let join_predicate = ConcretePredicate::Eq(
            Column::Physical(
                database
                    .get_column_id(concert_physical_table_id, "venue_id")
                    .unwrap(),
            ),
            Column::Physical(
                database
                    .get_column_id(venue_physical_table_id, "id")
                    .unwrap(),
            ),
        );

        let concert_table = Table::Physical(concert_physical_table_id);
        let venue_table = Table::Physical(venue_physical_table_id);
        let join = LeftJoin::new(concert_table, venue_table, join_predicate);

        let mut builder = SQLBuilder::new();
        join.build(&database, &mut builder);

        assert_binding!(
            builder.into_sql(),
            r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
        );
    }
}
