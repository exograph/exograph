// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Abstraction to allow updating rows in a table as well as related tables.
//!
//! In the following:
//! - Concert ids start at 1
//! - Artist ids start at 10
//! - ConcertArtist ids start at 100
//!
//! Here we want to (for concert with id 4)
//! - add a new ConcertArtist for an Artist with id 30 and assign rank of 2 and role of "main".
//! - update some characteristics of ConcertArtists 100 and 101
//! - remove ConcertArtist 110
//!
//! This allows us to execute GraphQL mutations like this:
//!
//! ```graphql
//! mutation {
//!   updateConcert(id: 4, data: {
//!     title: "new-title",
//!     concertArtists: {
//!       create: [{artist: {id: 30}, rank: 2, role: "main"}],
//!       update: [{id: 100, artist: {id: 10}, rank: 2}, {id: 101, artist: {id: 10}, role: "accompanying"}],
//!       delete: [{id: 110}]
//!     }
//!   }) {
//!     id
//!   }
//! }
//! ```
//!
//! Here, concert artists created will have their `concert_id` set to the id of the concert being
//! updated. Specifically, concert_artist will have its `concert_id` set to 4 (along with
//! user-provided values for artist id, `rank` and `role`).
//!
//! For update and delete, the concert id (4) will be used as a predicate in addition to the
//! user-provided predicates (id = 100 for update and id =  110 for delete). TODO: Should we
//! fail if the the combined predicate does not match any rows?

use crate::{sql::column::Column, ColumnId, OneToMany, TableId};

use super::{
    delete::AbstractDelete, insert::AbstractInsert, predicate::AbstractPredicate,
    select::AbstractSelect,
};

/// Abstract representation of an update statement.
///
/// An update may have nested create, update, and delete operations. This supports updating a tree of entities
/// starting at the root table. For example, while updating a concert, this allows adding a new concert-artist,
/// updating (say, role or rank) of an existing concert-artist, or deleting an existing concert-artist.
#[derive(Debug)]
pub struct AbstractUpdate {
    /// The table to update
    pub table_id: TableId,
    /// The predicate to filter rows.
    pub predicate: AbstractPredicate,

    /// The columns to update and their values for the `table`
    pub column_values: Vec<(ColumnId, Column)>,

    /// Nested updates
    pub nested_updates: Vec<NestedAbstractUpdate>,
    /// Nested inserts
    pub nested_inserts: Vec<NestedAbstractInsert>,
    /// Nested deletes
    pub nested_deletes: Vec<NestedAbstractDelete>,

    /// The selection to return
    pub selection: AbstractSelect,
}

/// In our example, the `update: [{id: 100, artist: {id: 10}, rank: 2}, {id: 101, artist: {id: 10}, role: "accompanying"}]` part
#[derive(Debug)]
pub struct NestedAbstractUpdate {
    /// The relation with the parent table. In our example, this would be `OneToMany { self_pk_column_id: concert.id, foreign_column_id: concert_artist.concert_id}`
    pub nesting_relation: OneToMany,
    /// The update to apply to the nested table
    pub update: AbstractUpdate,
}

/// In our example, the `create: [{artist: {id: 30}, rank: 2, role: "main"}]` part
#[derive(Debug)]
pub struct NestedAbstractInsert {
    /// Same as `NestedAbstractUpdate::relation_column_id`
    pub relation_column_id: ColumnId,
    /// The insert to apply to the nested table
    pub insert: AbstractInsert,
}

/// In our example, the `delete: [{id: 110}]` part
#[derive(Debug)]
pub struct NestedAbstractDelete {
    /// Same as `NestedAbstractUpdate::nesting_relation`
    pub nesting_relation: OneToMany,
    /// The delete to apply to the nested table
    pub delete: AbstractDelete,
}
