// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub mod column_default;
pub mod column_path;
pub mod database;
pub mod database_error;
pub mod function;
pub mod index_kind;
pub mod limit;
pub mod offset;
pub mod order;
pub mod physical_column;
pub mod physical_column_type;
pub mod physical_table;
pub mod predicate;
pub mod relation;
pub mod schema_object;
pub mod sql_bytes;
pub mod sql_param;
pub mod sql_param_container;
pub mod sql_value;
pub mod statement;
#[cfg(any(test, feature = "test-support"))]
pub mod test_database_builder;
pub mod vector;

#[cfg(feature = "bigdecimal")]
pub use pg_bigdecimal::BigDecimal;

// Re-export commonly used types at the crate root
pub use column_default::{
    ColumnAutoincrement, ColumnDefault, IdentityGeneration, UuidGenerationMethod,
};
pub use column_path::{ColumnPathLink, PhysicalColumnPath, RelationLink};
pub use database::Database;
pub use database::EnumId;
pub use database::TableId;
pub use database_error::DatabaseError;
pub use function::Function;
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
pub use predicate::{CaseSensitivity, NumericComparator, ParamEquality, Predicate};
pub use relation::{
    ManyToOne, ManyToOneId, OneToMany, OneToManyId, RelationColumnPair, RelationId,
};
pub use schema_object::SchemaObjectName;
pub use sql_bytes::SQLBytes;
pub use sql_param::SQLParam;
pub use sql_param_container::SQLParamContainer;
pub use statement::SchemaStatement;
pub use vector::{DEFAULT_VECTOR_SIZE, VectorDistanceFunction};
