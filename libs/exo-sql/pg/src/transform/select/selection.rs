// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Database;
use exo_sql_model::{
    AliasedSelectionElement, Selection, SelectionCardinality, SelectionElement,
    selection_level::SelectionLevel,
};

use crate::{
    Column, PgExtension, PgSelection, PgSelectionElement,
    core::json_agg::JsonAgg,
    core::json_object::{JsonObject, JsonObjectElement},
};

use crate::pg::Postgres;

pub enum SelectionSQL {
    Single(Column),
    Seq(Vec<Column>),
}

pub(crate) trait SelectionExt {
    fn to_pg_sql(
        self,
        selection_level: &SelectionLevel,
        select_transformer: &Postgres,
        database: &Database,
    ) -> SelectionSQL;

    fn selection_aggregate(
        self,
        selection_level: &SelectionLevel,
        select_transformer: &Postgres,
        database: &Database,
    ) -> Vec<Column>;
}

pub(crate) trait SelectionElementExt {
    fn to_pg_sql(
        self,
        selection_level: &SelectionLevel,
        transformer: &Postgres,
        database: &Database,
    ) -> Column;
}

impl SelectionExt for PgSelection {
    fn to_pg_sql(
        self,
        selection_level: &SelectionLevel,
        select_transformer: &Postgres,
        database: &Database,
    ) -> SelectionSQL {
        match self {
            Selection::Seq(seq) => SelectionSQL::Seq(
                seq.into_iter()
                    .map(
                        |AliasedSelectionElement {
                             alias: _alias,
                             column,
                         }| {
                            column.to_pg_sql(selection_level, select_transformer, database)
                        },
                    )
                    .collect(),
            ),
            Selection::Json(seq, cardinality) => {
                let object_elems = seq
                    .into_iter()
                    .map(|AliasedSelectionElement { alias, column }| {
                        JsonObjectElement::new(
                            alias.clone(),
                            column.to_pg_sql(selection_level, select_transformer, database),
                        )
                    })
                    .collect();

                let json_obj = Column::Extension(PgExtension::JsonObject(JsonObject(object_elems)));

                match cardinality {
                    SelectionCardinality::One => SelectionSQL::Single(json_obj),
                    SelectionCardinality::Many => SelectionSQL::Single(Column::Extension(
                        PgExtension::JsonAgg(JsonAgg(Box::new(json_obj))),
                    )),
                }
            }
        }
    }

    fn selection_aggregate(
        self,
        selection_level: &SelectionLevel,
        select_transformer: &Postgres,
        database: &Database,
    ) -> Vec<Column> {
        match self.to_pg_sql(selection_level, select_transformer, database) {
            SelectionSQL::Single(elem) => vec![elem],
            SelectionSQL::Seq(elems) => elems,
        }
    }
}

impl SelectionElementExt for PgSelectionElement {
    fn to_pg_sql(
        self,
        selection_level: &SelectionLevel,
        transformer: &Postgres,
        database: &Database,
    ) -> Column {
        match self {
            SelectionElement::Physical(column_id) => {
                let column = column_id.get_column(database);
                let foreign_table_alias =
                    selection_level.self_referencing_table_alias(column.table_id, database);

                Column::physical(column_id, foreign_table_alias)
            }
            SelectionElement::Function(function) => Column::Function(function.clone()),
            SelectionElement::Constant(s) => Column::Constant(s.clone()),
            SelectionElement::Object(elements) => {
                let elements = elements
                    .into_iter()
                    .map(|(alias, column)| {
                        JsonObjectElement::new(
                            alias.to_owned(),
                            column.to_pg_sql(selection_level, transformer, database),
                        )
                    })
                    .collect();
                Column::Extension(PgExtension::JsonObject(JsonObject(elements)))
            }
            SelectionElement::SubSelect(relation_id, select) => {
                let new_selection_level = selection_level.with_relation_id(relation_id);
                Column::SubSelect(Box::new(transformer.compute_select(
                    *select,
                    &new_selection_level,
                    false,
                    database,
                )))
            }
        }
    }
}
