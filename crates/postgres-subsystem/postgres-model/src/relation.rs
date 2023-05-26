// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::types::EntityFieldId;

use exo_sql::{ColumnId, PhysicalColumnPathLink};
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
    // - self_column_id: concerts.venue_id
    // - foreign_pk_column_id: venues.id
    // Concert.venue
    pub cardinality: RelationCardinality,
    pub foreign_pk_field_id: EntityFieldId,
    pub self_column_id: ColumnId,
    pub foreign_pk_column_id: ColumnId,
}

impl ManyToOneRelation {
    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        PhysicalColumnPathLink {
            self_column_id: self.self_column_id,
            linked_column_id: Some(self.foreign_pk_column_id),
        }
    }
}

/// Model for a one-to-many relation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OneToManyRelation {
    // For the `Venue.concerts` field (assuming Venue -> [Concert]), we will have:
    // - cardinality: Unbounded
    // - foreign_field_id: Concert.venue
    // - self_pk_column_id: venues.id
    // - foreign_column_id: concerts.venue_id
    pub cardinality: RelationCardinality,
    pub foreign_field_id: EntityFieldId,

    pub self_pk_column_id: ColumnId,

    /// This is a redundant information (we can get this from foreign_field_id using
    /// foreign_column_id.resolve(...).relation.<self-column-id>.unwrap()), since
    /// the type of `relation` is `PostgresRelation`, so at the type level we don't know
    /// if it's a `ManyToOne` or `OneToMany` relation and requires unwrapping.
    pub foreign_column_id: ColumnId,
}

impl OneToManyRelation {
    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        PhysicalColumnPathLink {
            self_column_id: self.self_pk_column_id,
            linked_column_id: Some(self.foreign_column_id),
        }
    }
}

impl PostgresRelation {
    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        match &self {
            PostgresRelation::Pk { column_id, .. } | PostgresRelation::Scalar { column_id, .. } => {
                PhysicalColumnPathLink::new(*column_id, None)
            }
            PostgresRelation::ManyToOne(relation) => relation.column_path_link(),
            PostgresRelation::OneToMany(relation) => relation.column_path_link(),
        }
    }
}
