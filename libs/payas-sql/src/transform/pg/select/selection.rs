use crate::{
    asql::selection::SelectionSQL,
    sql::{
        json_agg::JsonAgg,
        json_object::{JsonObject, JsonObjectElement},
        predicate::ConcretePredicate,
    },
    transform::{transformer::SelectTransformer, SelectionLevel},
    Column, ColumnSelection, Selection, SelectionCardinality, SelectionElement,
};

impl<'a> Selection<'a> {
    pub fn to_sql(&self, select_transformer: &impl SelectTransformer) -> SelectionSQL<'a> {
        match self {
            Selection::Seq(seq) => SelectionSQL::Seq(
                seq.iter()
                    .map(
                        |ColumnSelection {
                             alias: _alias,
                             column,
                         }| column.to_sql(select_transformer),
                    )
                    .collect(),
            ),
            Selection::Json(seq, cardinality) => {
                let object_elems = seq
                    .iter()
                    .map(|ColumnSelection { alias, column }| {
                        JsonObjectElement::new(alias.clone(), column.to_sql(select_transformer))
                    })
                    .collect();

                let json_obj = Column::JsonObject(JsonObject(object_elems));

                match cardinality {
                    SelectionCardinality::One => SelectionSQL::Single(json_obj),
                    SelectionCardinality::Many => {
                        SelectionSQL::Single(Column::JsonAgg(JsonAgg(Box::new(json_obj))))
                    }
                }
            }
        }
    }

    pub fn selection_aggregate(
        &self,
        select_transformer: &impl SelectTransformer,
    ) -> Vec<Column<'a>> {
        match self.to_sql(select_transformer) {
            SelectionSQL::Single(elem) => vec![elem],
            SelectionSQL::Seq(elems) => elems,
        }
    }
}

impl<'a> SelectionElement<'a> {
    pub fn to_sql(&self, database_kind: &impl SelectTransformer) -> Column<'a> {
        match self {
            SelectionElement::Physical(pc) => Column::Physical(pc),
            SelectionElement::Function {
                function_name,
                column,
            } => Column::Function {
                function_name: function_name.clone(),
                column,
            },
            SelectionElement::Constant(s) => Column::Constant(s.clone()),
            SelectionElement::Object(elements) => {
                let elements = elements
                    .iter()
                    .map(|(alias, column)| {
                        JsonObjectElement::new(alias.to_owned(), column.to_sql(database_kind))
                    })
                    .collect();
                Column::JsonObject(JsonObject(elements))
            }
            SelectionElement::Nested(relation, select) => {
                Column::SubSelect(Box::new(database_kind.to_select(
                    select,
                    relation.linked_column.map(|linked_column| {
                        ConcretePredicate::Eq(
                            Column::Physical(relation.self_column.0),
                            Column::Physical(linked_column.0),
                        )
                    }),
                    None,
                    SelectionLevel::Nested,
                )))
            }
        }
    }
}
