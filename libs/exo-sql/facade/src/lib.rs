// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! The core idea in this library is that of [AbstractOperation], which along with
//! its variants, allows declaring an intention of a database operation at a higher
//! level. It also offers [DatabaseBackend] (with [PgBackend] as the Postgres
//! implementation), which is responsible for transforming an [AbstractOperation]
//! into one or more SQL operations and executing them. This separation of intention
//! vs execution allows for simplified expression from the user of the library and
//! leaves out the details of the database operations.
//!
//! For example, consider [AbstractSelect]. It allows expressing the intention to
//! query data by specifying the root table, a predicate, and (potentially nested)
//! columns (among other things). It doesn't, however, express how to execute
//! the query; specifically, it doesn't specify any joins to be performed.
//! Similarly, [AbstractInsert] expresses an intention to insert logical rows
//! (columns into the root table as well as any referenced tables), but doesn't
//! specify how to go about doing so.
//!
//! To allow expressing complex operations such as predicates based on nested
//! elements, the library requires the use of [ColumnPath]s in the predicates and
//! order by expressions. A [ColumnPath] is a path from the root table of the
//! operation to the intended column. Similarly, to allow inserting nested elements,
//! the library requires expressing the columns to be inserted as
//! [InsertionElement]s, which abstracts over columns and nested elements.
//!
//! This is a facade crate that re-exports from the underlying sub-crates:
//! - `exo-sql-core`: generic data model types
//! - `exo-sql-model`: abstract SQL operations + transform traits
//! - `exo-sql-pg`: Postgres SQL types, generation, and transformation
//! - `exo-sql-pg-connect`: Postgres connection management + execution
//! - `exo-sql-pg-schema`: Postgres schema introspection, diff, migration

pub mod schema;

pub mod array_util {
    pub use exo_sql_pg::array_util::*;
}

pub mod column_default {
    pub use exo_sql_core::column_default::*;
}

pub mod database_error {
    pub use exo_sql_core::database_error::*;
}

#[cfg(feature = "test-support")]
pub mod testing {
    pub use exo_sql_pg_connect::testing::*;
}

pub use exo_sql_model::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractOrderBy, AbstractOrderByExpr,
    AbstractPredicate, AbstractPredicateExt, AbstractSelect, AbstractUpdate,
    AliasedSelectionElement, ColumnPath, ColumnValuePair, InsertionElement, InsertionRow,
    NestedAbstractDelete, NestedAbstractInsert, NestedAbstractInsertSet, NestedAbstractUpdate,
    NestedInsertion, Selection, SelectionCardinality, SelectionElement,
};

pub use exo_sql_core::{ColumnPathLink, PhysicalColumnPath};

pub use exo_sql_model::DatabaseBackend;
pub use exo_sql_pg_connect::PgBackend;
pub use exo_sql_pg_connect::{
    TransactionHolder,
    connect::creation::{Connect, TransactionMode},
    connect::database_client::DatabaseClient,
    connect::database_client_manager::DatabaseClientManager,
};

pub use exo_sql_pg::column::Column;
pub use exo_sql_pg::{PgColumnTypeExt, ensure_registry_initialized};

pub use exo_sql_pg::physical_column_type::{
    ArrayColumnType, BlobColumnType, BooleanColumnType, DateColumnType, EnumColumnType, FloatBits,
    FloatColumnType, IntBits, IntColumnType, JsonColumnType, NumericColumnType, PhysicalColumnType,
    PhysicalColumnTypeExt, StringColumnType, TimeColumnType, TimestampColumnType, UuidColumnType,
    VectorColumnType, physical_column_type_from_string,
};

// Types from pg (moved from core)
pub use exo_sql_pg::{
    CaseSensitivity, DEFAULT_VECTOR_SIZE, Function, HNWSParams, IndexKind, NumericComparator,
    ParamEquality, PgAbstractDelete, PgAbstractInsert, PgAbstractOperation, PgAbstractOrderBy,
    PgAbstractOrderByExtension, PgAbstractPredicate, PgAbstractSelect, PgAbstractUpdate,
    PgAliasedSelectionElement, PgColumnExtension, PgColumnPath, PgExtension, PgFunctionExtension,
    PgInsertionElement, PgInsertionRow, PgNestedAbstractDelete, PgNestedAbstractInsert,
    PgNestedAbstractInsertSet, PgNestedAbstractUpdate, PgOrderByExtension, PgPredicateExtension,
    PgSelectionElement, Predicate, VectorDistanceFunction, sql_param_container::SQLParamContainer,
};

// Types that remain in core
pub use exo_sql_core::{
    ColumnId, ColumnReference, Database, DatabaseError, Limit, ManyToOne, ManyToOneId, Offset,
    OneToMany, OneToManyId, Ordering, PhysicalColumn, PhysicalEnum, PhysicalIndex, PhysicalTable,
    RelationColumnPair, RelationId, SchemaObjectName, TableId,
    physical_column::{get_mto_relation_for_columns, get_otm_relation_for_columns},
};

#[cfg(feature = "bigdecimal")]
pub use exo_sql_pg::BigDecimal;
