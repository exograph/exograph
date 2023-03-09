use maybe_owned::MaybeOwned;

use super::{predicate::ConcretePredicate, table::Table, ExpressionBuilder, SQLBuilder};

/// Represents a join between two tables. Currently, supports only left join.
#[derive(Debug, PartialEq)]
pub struct LeftJoin<'a> {
    /// The left table in the join such as `concerts`.
    left: Box<Table<'a>>,
    /// The right table in the join such as `venues`.
    right: Box<Table<'a>>,
    /// The join predicate such as `concerts.venue_id = venues.id`.
    predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
}

impl<'a> LeftJoin<'a> {
    pub fn new(
        left: Table<'a>,
        right: Table<'a>,
        predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    ) -> Self {
        LeftJoin {
            left: Box::new(left),
            right: Box::new(right),
            predicate,
        }
    }

    pub fn left(&self) -> &Table<'a> {
        &self.left
    }
}

impl ExpressionBuilder for LeftJoin<'_> {
    /// Build expression of the form `<left> LEFT JOIN <right> ON <predicate>`.
    fn build(&self, builder: &mut SQLBuilder) {
        self.left.build(builder);
        builder.push_str(" LEFT JOIN ");
        self.right.build(builder);
        builder.push_str(" ON ");
        self.predicate.build(builder);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sql::physical_column::{IntBits, PhysicalColumn, PhysicalColumnType},
        PhysicalTable,
    };

    use super::*;

    #[test]
    fn basic_join() {
        let concert_physical_table = PhysicalTable {
            name: "concerts".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "concerts".to_string(),
                    name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: true,
                    ..Default::default()
                },
                PhysicalColumn {
                    table_name: "concerts".to_string(),
                    name: "venue_id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    ..Default::default()
                },
            ],
        };

        let venue_physical_table = PhysicalTable {
            name: "venues".to_string(),
            columns: vec![
                PhysicalColumn {
                    table_name: "venues".to_string(),
                    name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    ..Default::default()
                },
                PhysicalColumn {
                    table_name: "venues".to_string(),
                    name: "capacity".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    ..Default::default()
                },
            ],
        };

        let concert_table = Table::Physical(&concert_physical_table);
        let venue_table = Table::Physical(&venue_physical_table);
        let join_predicate = ConcretePredicate::Eq(
            concert_physical_table.get_column("venue_id").unwrap(),
            venue_physical_table.get_column("id").unwrap(),
        )
        .into();
        let join = LeftJoin::new(concert_table, venue_table, join_predicate);

        let mut builder = SQLBuilder::new();
        join.build(&mut builder);

        assert_binding!(
            builder.into_sql(),
            r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
        );
    }
}
