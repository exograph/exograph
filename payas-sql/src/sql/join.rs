use maybe_owned::MaybeOwned;

use super::{predicate::Predicate, Expression, ParameterBinding, Table};

#[derive(Debug, PartialEq)]
pub struct Join<'a> {
    left: Box<Table<'a>>,
    right: Box<Table<'a>>,
    predicate: MaybeOwned<'a, Predicate<'a>>,
}

impl<'a> Join<'a> {
    pub fn new(
        left: Table<'a>,
        right: Table<'a>,
        predicate: MaybeOwned<'a, Predicate<'a>>,
    ) -> Self {
        Join {
            left: Box::new(left),
            right: Box::new(right),
            predicate,
        }
    }
}

impl Expression for Join<'_> {
    fn binding(&self, expression_context: &mut super::ExpressionContext) -> ParameterBinding {
        let left_binding = self.left.binding(expression_context);
        let right_binding = self.right.binding(expression_context);
        let predicate_binding = self.predicate.binding(expression_context);

        let mut params = left_binding.params;
        params.extend(right_binding.params);
        params.extend(predicate_binding.params);

        ParameterBinding {
            stmt: format!(
                "{} LEFT JOIN {} ON {}",
                left_binding.stmt, right_binding.stmt, predicate_binding.stmt
            ),
            params,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sql::{
        column::{IntBits, PhysicalColumn, PhysicalColumnType},
        ExpressionContext, PhysicalTable,
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
                    is_pk: false,
                    is_autoincrement: false,
                },
                PhysicalColumn {
                    table_name: "concerts".to_string(),
                    column_name: "venue_id".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: false,
                    is_autoincrement: false,
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
                    is_pk: false,
                    is_autoincrement: false,
                },
                PhysicalColumn {
                    table_name: "venues".to_string(),
                    column_name: "capacity".to_string(),
                    typ: PhysicalColumnType::Int { bits: IntBits::_16 },
                    is_pk: false,
                    is_autoincrement: false,
                },
            ],
        };

        let concert_table = Table::Physical(&concert_physical_table);
        let venue_table = Table::Physical(&venue_physical_table);
        let join_predicate = Predicate::Eq(
            concert_physical_table
                .get_column("venue_id")
                .unwrap()
                .into(),
            venue_physical_table.get_column("id").unwrap().into(),
        )
        .into();
        let join = Join::new(concert_table, venue_table, join_predicate);

        let mut expression_context = ExpressionContext::default();
        let binding = join.binding(&mut expression_context);

        assert_binding!(
            &binding,
            r#""concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
        );
    }
}
