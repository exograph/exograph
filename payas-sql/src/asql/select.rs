use crate::{
    asql::{
        column_path::{ColumnPath, ColumnPathLink},
        selection::SelectionSQL,
        util,
    },
    sql::{predicate::Predicate, Limit, Offset, PhysicalTable, Select},
};

use super::{order_by::AbstractOrderBy, predicate::AbstractPredicate, selection::Selection};

pub struct AbstractSelect<'a> {
    pub table: &'a PhysicalTable,
    pub selection: Selection<'a>,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub order_by: Option<AbstractOrderBy<'a>>,
    pub offset: Option<Offset>,
    pub limit: Option<Limit>,
}

impl<'a> AbstractSelect<'a> {
    pub fn to_sql(&'a self) -> Select<'a> {
        fn column_path_owned<'a>(
            column_paths: Vec<&ColumnPath<'a>>,
        ) -> Vec<Vec<ColumnPathLink<'a>>> {
            column_paths
                .into_iter()
                .filter_map(|path| match path {
                    ColumnPath::Physical(links) => Some(links.to_vec()),
                    ColumnPath::Literal(_) => None,
                })
                .collect()
        }

        let predicate_column_paths: Vec<Vec<ColumnPathLink>> = self
            .predicate
            .as_ref()
            .map(|predicate| column_path_owned(predicate.column_paths()))
            .unwrap_or_else(Vec::new);

        let order_by_column_paths = self
            .order_by
            .as_ref()
            .map(|ob| column_path_owned(ob.column_paths()))
            .unwrap_or_else(Vec::new);

        let columns_paths = predicate_column_paths
            .into_iter()
            .chain(order_by_column_paths.into_iter())
            .collect();

        let join = util::compute_join(self.table, columns_paths);

        let columns = match self.selection.to_sql() {
            SelectionSQL::Single(elem) => vec![elem.into()],
            SelectionSQL::Seq(elems) => elems.into_iter().map(|elem| elem.into()).collect(),
        };

        Select {
            underlying: join,
            columns,
            predicate: self
                .predicate
                .as_ref()
                .map(|p| p.predicate().into())
                .unwrap_or_else(|| Predicate::True.into()),
            order_by: self.order_by.as_ref().map(|ob| ob.order_by()),
            offset: self.offset.clone(),
            limit: self.limit.clone(),
            top_level_selection: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        asql::{
            column_path::{ColumnPath, ColumnPathLink},
            predicate::AbstractPredicate,
            selection::{Selection, SelectionElement},
            test_util::TestSetup,
        },
        sql::ExpressionContext,
    };

    use super::AbstractSelect;
    use crate::sql::Expression;

    #[test]
    fn simple_selection() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Seq(vec![SelectionElement::Physical(concerts_id_column)]),
                    predicate: None,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_sql();
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(binding, r#"select "concerts"."id" from "concerts""#);
            },
        );
    }

    #[test]
    fn simple_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let concert_id_path = ColumnPath::Physical(vec![ColumnPathLink {
                    self_column: (concerts_id_column, concerts_table),
                    linked_column: None,
                }]);
                let literal = ColumnPath::Literal(Box::new(5));

                let predicate = AbstractPredicate::Eq(concert_id_path, literal);
                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Seq(vec![SelectionElement::Physical(concerts_id_column)]),
                    predicate: Some(predicate),
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_sql();
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select "concerts"."id" from "concerts" WHERE "concerts"."id" = $1"#,
                    5
                );
            },
        );
    }

    #[test]
    fn non_nested_json() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Json(
                        vec![("id", SelectionElement::Physical(concerts_id_column))],
                        true,
                    ),
                    predicate: None,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_sql();
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select coalesce(json_agg(json_build_object('id', "concerts"."id")), '[]'::json)::text from "concerts""#
                );
            },
        );
    }

    #[test]
    fn nested_json() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 venues_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Json(
                        vec![
                            ("id", SelectionElement::Physical(concerts_id_column)),
                            (
                                "venue",
                                SelectionElement::Compound(Selection::Json(
                                    vec![("id", SelectionElement::Physical(venues_id_column))],
                                    false,
                                )),
                            ),
                        ],
                        true,
                    ),
                    predicate: None,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_sql();
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select coalesce(json_agg(json_build_object('id', "concerts"."id", 'venue', (select json_build_object('id', "venues"."id") from "venues" WHERE "concerts"."venueid" = "venues"."id"))), '[]'::json)::text from "concerts""#
                );
                //select coalesce(json_agg(json_build_object('id', "concerts"."id", 'venue', json_build_object('id', "venues"."id"))), '[]'::json)::text from "concerts"
            },
        );
    }
}
