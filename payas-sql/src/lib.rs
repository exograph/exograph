#![doc = include_str!("../README.md")]
pub mod spec;
#[macro_use]
mod sql;
mod asql;
mod transform;

/// Public types at the root level of this crate
pub use asql::{
    abstract_operation::AbstractOperation,
    column_path::{ColumnPath, ColumnPathLink},
    database_executor::DatabaseExecutor,
    delete::AbstractDelete,
    insert::{AbstractInsert, ColumnValuePair, InsertionElement, InsertionRow, NestedInsertion},
    order_by::AbstractOrderBy,
    predicate::AbstractPredicate,
    select::AbstractSelect,
    selection::{
        ColumnSelection, NestedElementRelation, Selection, SelectionCardinality, SelectionElement,
    },
    update::{AbstractUpdate, NestedAbstractDelete, NestedAbstractInsert, NestedAbstractUpdate},
};

pub use sql::{
    array_util::{self, ArrayEntry},
    column::{Column, FloatBits, IntBits, PhysicalColumn, PhysicalColumnType},
    database::Database,
    limit::Limit,
    offset::Offset,
    order::Ordering,
    physical_table::PhysicalTable,
    predicate::Predicate,
    SQLBytes, SQLParam,
};
