// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Database, RelationId};

pub mod delete_transformer;
mod insert_transformer;
mod order_by_transformer;
mod predicate_transformer;
mod select;
mod update_transformer;

pub struct Postgres {}

#[derive(Debug, Clone)]
pub enum SelectionLevel {
    /// Top level selection
    TopLevel,
    /// Nested sub selection, which each element representing the relation between parent and child selection
    /// For example, if we have a query like: `concerts { venue { .. }}`, the selection level for the venue
    /// selection will be `Nested(vec![RelationId::ManyToOne(<venues.id, concerts.venue_id>)])`.
    Nested(Vec<RelationId>),
}

impl SelectionLevel {
    fn is_top_level(&self) -> bool {
        matches!(self, SelectionLevel::TopLevel)
    }

    fn with_relation_id(&self, relation_id: RelationId) -> Self {
        match self {
            SelectionLevel::TopLevel => SelectionLevel::Nested(vec![relation_id]),
            SelectionLevel::Nested(relation_ids) => {
                let mut relation_ids = relation_ids.clone();
                relation_ids.push(relation_id);
                SelectionLevel::Nested(relation_ids)
            }
        }
    }

    pub fn tail_relation_id(&self) -> Option<&RelationId> {
        match self {
            SelectionLevel::TopLevel => None,
            SelectionLevel::Nested(relation_ids) => relation_ids.last(),
        }
    }

    pub fn alias(&self, database: &Database) -> Option<String> {
        match self {
            SelectionLevel::TopLevel => None,
            SelectionLevel::Nested(relation_ids) => {
                Some(relation_ids.iter().fold(String::new(), |acc, relation_id| {
                    let foreign_table_id = match relation_id {
                        RelationId::ManyToOne(r) => r.deref(database).self_column_id.table_id,
                        RelationId::OneToMany(r) => r.deref(database).self_pk_column_id.table_id,
                    };
                    let name = &database.get_table(foreign_table_id).name;
                    join_alias_components(&acc, name)
                }))
            }
        }
    }
}

const ALIAS_SEPARATOR: &str = "$";

pub fn make_alias(name: &str, context_name: &Option<String>) -> String {
    match context_name {
        Some(ref context_name) => join_alias_components(context_name, name),
        None => name.to_owned(),
    }
}

fn join_alias_components(context_name: &str, name: &str) -> String {
    format!("{context_name}{ALIAS_SEPARATOR}{name}")
}
