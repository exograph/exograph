// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub mod operation;
pub mod physical;

// Shared types used by both physical and operation modules
pub mod limit;
pub mod offset;
pub mod order;
pub mod statement;

// Re-export physical modules at crate root for backward compatibility
pub use physical::column_default;
pub use physical::column_path;
pub use physical::database;
pub use physical::database_error;
pub use physical::index_kind;
pub use physical::physical_column;
pub use physical::physical_column_type;
pub use physical::physical_table;
pub use physical::relation;
pub use physical::schema_object;
pub use physical::vector;

// Re-export commonly used types at the crate root
pub use column_default::{
    ColumnAutoincrement, ColumnDefault, IdentityGeneration, UuidGenerationMethod,
};
pub use column_path::{ColumnPathLink, PhysicalColumnPath, RelationLink};
pub use database::Database;
pub use database::EnumId;
pub use database::TableId;
pub use database_error::DatabaseError;
pub use index_kind::{HNWSParams, IndexKind};
pub use limit::Limit;
pub use offset::Offset;
pub use order::Ordering;
pub use physical_column::ColumnId;
pub use physical_column::ColumnReference;
pub use physical_column::PhysicalColumn;
pub use physical_table::PhysicalEnum;
pub use physical_table::PhysicalIndex;
pub use physical_table::PhysicalTable;
pub use relation::{
    ManyToOne, ManyToOneId, OneToMany, OneToManyId, RelationColumnPair, RelationId,
};
pub use schema_object::SchemaObjectName;
pub use statement::SchemaStatement;
pub use vector::{DEFAULT_VECTOR_SIZE, VectorDistanceFunction};
