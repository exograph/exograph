// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::types::EntityFieldId;

use exo_sql::{
    ColumnId, Database, ManyToOneRelationId, OneToManyRelationId, PhysicalColumnPathLink,
};
use serde::{Deserialize, Serialize};

// We model one-to-one (more precisely one-to-one_or_zero and one_or_zero-to-one) relations as
// a OneToMany and ManyToOne relation (respectively), so that we can share most of the logic to
// build queries etc. We use RelationCardinality to distinguish between these two cases.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RelationCardinality {
    Optional,  // The cardinality of a "one-to-one" relation
    Unbounded, // The cardinality for a "many" relationship.
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PostgresRelation {
    Pk { column_id: ColumnId },
    Scalar { column_id: ColumnId },
    ManyToOne(ManyToOneRelation),
    OneToMany(OneToManyRelation),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManyToOneRelation {
    // For the `Concert.venue` field (assuming [Concert] -> Venue), we will have:
    // - cardinality: Unbounded
    // - foreign_pk_field_id: Venue.id
    // - relation_id.self_column_id: concerts.venue_id
    // - relation_id.foreign_pk_column_id: venues.id
    pub cardinality: RelationCardinality,
    pub foreign_pk_field_id: EntityFieldId,
    pub relation_id: ManyToOneRelationId,
}

impl ManyToOneRelation {
    pub fn column_path_link(&self, database: &Database) -> PhysicalColumnPathLink {
        let relation = database.get_relation(self.relation_id);
        relation.column_path_link()
    }
}

/// Model for a one-to-many relation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OneToManyRelation {
    // For the `Venue.concerts` field (assuming Venue -> [Concert]), we will have:
    // - cardinality: Unbounded
    // - foreign_field_id: Concert.venue
    // - relation_id.self_pk_column_id: venues.id
    // - relation_id.foreign_column_id: concerts.venue_id
    pub cardinality: RelationCardinality,
    pub foreign_field_id: EntityFieldId,
    pub relation_id: OneToManyRelationId,
}

impl OneToManyRelation {
    pub fn column_path_link(&self, database: &Database) -> PhysicalColumnPathLink {
        let relation = database.get_relation(self.relation_id.underlying).flipped();
        relation.column_path_link()
    }
}

impl PostgresRelation {
    pub fn column_path_link(&self, database: &Database) -> PhysicalColumnPathLink {
        match &self {
            PostgresRelation::Pk { column_id, .. } | PostgresRelation::Scalar { column_id, .. } => {
                PhysicalColumnPathLink::Leaf(*column_id)
            }
            PostgresRelation::ManyToOne(relation) => relation.column_path_link(database),
            PostgresRelation::OneToMany(relation) => relation.column_path_link(database),
        }
    }
}
