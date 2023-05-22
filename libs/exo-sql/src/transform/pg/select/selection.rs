// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    sql::{
        json_agg::JsonAgg,
        json_object::{JsonObject, JsonObjectElement},
        predicate::ConcretePredicate,
    },
    transform::pg::{Postgres, SelectionLevel},
    AliasedSelectionElement, Column, Database, Selection, SelectionCardinality, SelectionElement,
};

pub enum SelectionSQL {
    Single(Column),
    Seq(Vec<Column>),
}

impl Selection {
    pub fn to_sql(&self, select_transformer: &Postgres, database: &Database) -> SelectionSQL {
        match self {
            Selection::Seq(seq) => SelectionSQL::Seq(
                seq.iter()
                    .map(
                        |AliasedSelectionElement {
                             alias: _alias,
                             column,
                         }| column.to_sql(select_transformer, database),
                    )
                    .collect(),
            ),
            Selection::Json(seq, cardinality) => {
                let object_elems = seq
                    .iter()
                    .map(|AliasedSelectionElement { alias, column }| {
                        JsonObjectElement::new(
                            alias.clone(),
                            column.to_sql(select_transformer, database),
                        )
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
        select_transformer: &Postgres,
        database: &Database,
    ) -> Vec<Column> {
        match self.to_sql(select_transformer, database) {
            SelectionSQL::Single(elem) => vec![elem],
            SelectionSQL::Seq(elems) => elems,
        }
    }
}

impl SelectionElement {
    pub fn to_sql(&self, transformer: &Postgres, database: &Database) -> Column {
        match self {
            SelectionElement::Physical(column_id) => Column::Physical(*column_id),
            SelectionElement::Function {
                function_name,
                column_id,
            } => Column::Function {
                function_name: function_name.clone(),
                column_id: *column_id,
            },
            SelectionElement::Constant(s) => Column::Constant(s.clone()),
            SelectionElement::Object(elements) => {
                let elements = elements
                    .iter()
                    .map(|(alias, column)| {
                        JsonObjectElement::new(
                            alias.to_owned(),
                            column.to_sql(transformer, database),
                        )
                    })
                    .collect();
                Column::JsonObject(JsonObject(elements))
            }
            SelectionElement::SubSelect(relation, select) => {
                Column::SubSelect(Box::new(transformer.compute_select(
                    select,
                    relation.linked_column_id.map(|linked_column_id| {
                        ConcretePredicate::Eq(
                            Column::Physical(relation.self_column_id),
                            Column::Physical(linked_column_id),
                        )
                    }),
                    SelectionLevel::Nested,
                    false,
                    database,
                )))
            }
        }
    }
}
