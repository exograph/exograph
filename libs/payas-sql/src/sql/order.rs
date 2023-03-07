use super::{column::PhysicalColumn, Expression, SQLBuilder};
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Ordering {
    Asc,
    Desc,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OrderByElement<'a>(pub &'a PhysicalColumn, pub Ordering);

#[derive(Debug, PartialEq, Eq)]
pub struct OrderBy<'a>(pub Vec<OrderByElement<'a>>);

impl<'a> OrderByElement<'a> {
    pub fn new(column: &'a PhysicalColumn, ordering: Ordering) -> Self {
        Self(column, ordering)
    }
}

impl<'a> Expression for OrderByElement<'a> {
    fn binding(&self, builder: &mut SQLBuilder) {
        self.0.binding(builder);
        builder.push(' ');
        if self.1 == Ordering::Asc {
            builder.push_str("ASC");
        } else {
            builder.push_str("DESC");
        }
    }
}

impl<'a> Expression for OrderBy<'a> {
    fn binding(&self, builder: &mut SQLBuilder) {
        builder.push_str("ORDER BY ");
        builder.push_elems(&self.0, ", ");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sql::column::{IntBits, PhysicalColumn, PhysicalColumnType};

    #[test]
    fn single() {
        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            ..Default::default()
        };

        let order_by = OrderBy(vec![OrderByElement::new(&age_col, Ordering::Desc)]);

        assert_binding!(order_by.into_sql(), r#"ORDER BY "people"."age" DESC"#);
    }

    #[test]
    fn multiple() {
        let name_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "name".to_string(),
            typ: PhysicalColumnType::String { length: None },
            ..Default::default()
        };

        let age_col = PhysicalColumn {
            table_name: "people".to_string(),
            column_name: "age".to_string(),
            typ: PhysicalColumnType::Int { bits: IntBits::_16 },
            ..Default::default()
        };

        {
            let order_by = OrderBy(vec![
                OrderByElement::new(&name_col, Ordering::Asc),
                OrderByElement::new(&age_col, Ordering::Desc),
            ]);

            assert_binding!(
                order_by.into_sql(),
                r#"ORDER BY "people"."name" ASC, "people"."age" DESC"#
            );
        }

        // Reverse the order and it should be reflected in the statement
        {
            let order_by = OrderBy(vec![
                OrderByElement::new(&age_col, Ordering::Desc),
                OrderByElement::new(&name_col, Ordering::Asc),
            ]);

            assert_binding!(
                order_by.into_sql(),
                r#"ORDER BY "people"."age" DESC, "people"."name" ASC"#
            );
        }
    }
}
