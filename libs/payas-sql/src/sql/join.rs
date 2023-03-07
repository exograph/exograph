use maybe_owned::MaybeOwned;

use super::{predicate::ConcretePredicate, table::TableQuery, Expression, SQLBuilder};

/// Represents a join between two tables. Currently, supports only left join.
#[derive(Debug, PartialEq)]
pub struct Join<'a> {
    left: Box<TableQuery<'a>>,
    right: Box<TableQuery<'a>>,
    predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
}

impl<'a> Join<'a> {
    pub fn new(
        left: TableQuery<'a>,
        right: TableQuery<'a>,
        predicate: MaybeOwned<'a, ConcretePredicate<'a>>,
    ) -> Self {
        Join {
            left: Box::new(left),
            right: Box::new(right),
            predicate,
        }
    }

    pub fn left(&self) -> &TableQuery<'a> {
        &self.left
    }
}

impl Expression for Join<'_> {
    fn binding(&self, builder: &mut SQLBuilder) {
        self.left.binding(builder);
        builder.push_str(" LEFT JOIN ");
        self.right.binding(builder);
        builder.push_str(" ON ");
        self.predicate.binding(builder);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sql::column::{IntBits, PhysicalColumn, PhysicalColumnType},
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
                    column_name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: true,
                    ..Default::default()
                },
                PhysicalColumn {
                    table_name: "concerts".to_string(),
                    column_name: "venue_id".to_string(),
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
                    column_name: "id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    ..Default::default()
                },
                PhysicalColumn {
                    table_name: "venues".to_string(),
                    column_name: "capacity".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    ..Default::default()
                },
            ],
        };

        let concert_table = TableQuery::Physical(&concert_physical_table);
        let venue_table = TableQuery::Physical(&venue_physical_table);
        let join_predicate = ConcretePredicate::Eq(
            concert_physical_table.get_column("venue_id").unwrap(),
            venue_physical_table.get_column("id").unwrap(),
        )
        .into();
        let join = Join::new(concert_table, venue_table, join_predicate);

        let mut builder = SQLBuilder::new();
        join.binding(&mut builder);

        assert_binding!(
            builder.into_sql(),
            r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
        );
    }
}
