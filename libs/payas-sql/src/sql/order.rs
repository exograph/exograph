use super::{column::PhysicalColumn, Expression, ParameterBinding};
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Ordering {
    Asc,
    Desc,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OrderBy<'a>(pub Vec<(&'a PhysicalColumn, Ordering)>);

impl<'a> Expression for OrderBy<'a> {
    fn binding(&self) -> ParameterBinding {
        let exprs = self
            .0
            .iter()
            .map(|(column, order)| ParameterBinding::OrderByElement(column, *order))
            .collect();

        ParameterBinding::OrderBy(exprs)
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

        let order_by = OrderBy(vec![(&age_col, Ordering::Desc)]);

        let binding = order_by.binding();

        assert_binding!(binding, r#"ORDER BY "people"."age" DESC"#);
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
            let order_by = OrderBy(vec![(&name_col, Ordering::Asc), (&age_col, Ordering::Desc)]);

            let binding = order_by.binding();

            assert_binding!(
                binding,
                r#"ORDER BY "people"."name" ASC, "people"."age" DESC"#
            );
        }

        // Reverse the order and it should be reflected in the statement
        {
            let order_by = OrderBy(vec![(&age_col, Ordering::Desc), (&name_col, Ordering::Asc)]);

            let binding = order_by.binding();

            assert_binding!(
                binding,
                r#"ORDER BY "people"."age" DESC, "people"."name" ASC"#
            );
        }
    }
}
