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
//! level. It also offers [DatabaseExecutor], which is responsible for transforming
//! an [AbstractOperation] into one or more SQL operations and executing them. This
//! separation of intention vs execution allows for simplified expression from the
//! user of the library and leaves out the details of the database operations.
//! Although, currently it focuses solely on Postgres support, it should be easy to
//! extend to other databases.
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
//! - `exo-sql-pg-core`: Postgres SQL types + generation
//! - `exo-sql-pg-transform`: Postgres transform implementation
//! - `exo-sql-pg-connect`: Postgres connection management + execution
//! - `exo-sql-pg-schema`: Postgres schema introspection, diff, migration

pub mod schema;

pub mod database_error {
    pub use exo_sql_core::database_error::*;
}

#[cfg(feature = "test-support")]
pub mod testing {
    pub use exo_sql_pg_connect::testing::*;
}

pub mod array_util {
    pub use exo_sql_pg_core::array_util::*;
}

pub use exo_sql_model::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractOrderBy, AbstractOrderByExpr,
    AbstractPredicate, AbstractPredicateExt, AbstractSelect, AbstractUpdate,
    AliasedSelectionElement, ColumnPath, ColumnValuePair, InsertionElement, InsertionRow,
    NestedAbstractDelete, NestedAbstractInsert, NestedAbstractInsertSet, NestedAbstractUpdate,
    NestedInsertion, Selection, SelectionCardinality, SelectionElement,
};

pub use exo_sql_core::{ColumnPathLink, PhysicalColumnPath};

mod database_executor;

pub use database_executor::DatabaseExecutor;
pub use exo_sql_pg_connect::{
    TransactionHolder,
    connect::creation::{Connect, TransactionMode},
    connect::database_client::DatabaseClient,
    connect::database_client_manager::DatabaseClientManager,
};

pub use exo_sql_pg_core::column::Column;
pub use exo_sql_pg_core::{PgColumnType, PgColumnTypeExt, as_pg_column_type, to_pg_array_type};

pub use exo_sql_core::{
    ColumnId, ColumnReference, Database, DatabaseError, Function, Limit, ManyToOne, ManyToOneId,
    Offset, OneToMany, OneToManyId, Ordering, ParamEquality, PhysicalColumn, PhysicalEnum,
    PhysicalIndex, PhysicalTable, Predicate, RelationColumnPair, RelationId, SQLBytes, SQLParam,
    SQLParamContainer, SchemaObjectName, TableId,
    physical_column::{get_mto_relation_for_columns, get_otm_relation_for_columns},
    physical_column_type::{
        ArrayColumnType, BlobColumnType, BooleanColumnType, DateColumnType, EnumColumnType,
        FloatBits, FloatColumnType, IntBits, IntColumnType, JsonColumnType, NumericColumnType,
        PhysicalColumnType, PhysicalColumnTypeExt, StringColumnType, TimeColumnType,
        TimestampColumnType, UuidColumnType, VectorColumnType,
    },
    predicate::{CaseSensitivity, NumericComparator},
    vector::{DEFAULT_VECTOR_SIZE, VectorDistanceFunction},
};

#[cfg(feature = "bigdecimal")]
pub use exo_sql_core::BigDecimal;
