use maybe_owned::MaybeOwned;

use super::{predicate::ConcretePredicate, table::TableQuery, Expression, ParameterBinding};

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
    fn binding(&self) -> ParameterBinding {
        let left_expr = self.left.binding();
        let right_expr = self.right.binding();
        let predicate_expr = self.predicate.binding();

        ParameterBinding::LeftJoin(
            Box::new(left_expr),
            Box::new(right_expr),
            Box::new(predicate_expr),
        )
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

        let binding = join.binding();

        assert_binding!(
            binding,
            r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
        );
    }
}
