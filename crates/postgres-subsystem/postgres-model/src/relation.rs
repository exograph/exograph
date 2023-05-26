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
    // Concert.venue
    pub cardinality: RelationCardinality,
    pub foreign_field_id: EntityFieldId, // foreign field id (e.g. Venue.id)
    pub column_id: ColumnId,             // self column id (e.g. concerts.venue_id)
    pub foreign_column_id: ColumnId,     // foreign column id (e.g. venues.id)
}

impl ManyToOneRelation {
    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        PhysicalColumnPathLink {
            self_column_id: self.column_id,
            linked_column_id: Some(self.foreign_column_id),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OneToManyRelation {
    // In case of Venue -> [Concert] and the enclosing type is `Venue`, we will have:
    // - other_type_id: Concert
    // - cardinality: Unbounded
    // - column_id_path_link: (self_column_id: venues.id, linked_column_id: concerts.venue_id)

    // Venue.concerts
    pub cardinality: RelationCardinality,
    pub foreign_field_id: EntityFieldId, // foreign field id (e.g. Concert.venue)

    pub pk_column_id: ColumnId,      // self pk column id (e.g. venues.id)
    pub foreign_column_id: ColumnId, // foreign column id (e.g. concerts.venue_id)
}

impl OneToManyRelation {
    pub fn column_path_link(&self) -> PhysicalColumnPathLink {
        PhysicalColumnPathLink {
            self_column_id: self.pk_column_id,
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
