/// The core idea in this library is that of [AbstractOperation], which along with
/// its variants, allows declaring an intention of a database operation at a higher
/// level. It also offers [DatabaseExecutor], which is responsible for transforming
/// an [AbstractOperation] into one or more SQL operations and executing them. This
/// separation of intention vs execution allows for simplified expression from the
/// user of the library and leaves out the details of the database operations.
/// Although, currently it focuses solely on Postgres support, it should be easy to
/// extend to other databases.
///
/// For example, consider [AbstractSelect]. It allows expressing the intention to
/// query data by specifying the root table, a predicate, and (potentially nested)
/// columns (among other things). It doesn't, however, express how to execute
/// the query; specifically, it doesn't specify any joins to be performed.
/// Similarly, [AbstractInsert] expresses an intention to insert logical rows
/// (columns into the root table as well as any referenced tables), but doesn't
/// specify how to go about doing so.
///
/// To allow expressing complex operations such as predicates based on nested
/// elements, the library requires the use of [ColumnPath]s in the predicates and
/// order by expressions. A [ColumnPath] is a path from the root table of the
/// operation to the intended column. Similarly, to allow inserting nested elements,
/// the library requires expressing the columns to be inserted as
/// [InsertionElement]s, which abstracts over columns and nested elements.
///
/// This crate also contains, but doesn't expose, lower level primitives for SQL
/// operations.
pub mod schema;
#[macro_use]
mod sql;
mod asql;
mod transform;

pub mod database_error;

/// Public types at the root level of this crate
pub use asql::{
    abstract_operation::AbstractOperation,
    column_path::{ColumnPath, ColumnPathLink},
    database_executor::{DatabaseExecutor, TransactionHolder},
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
    column::Column,
    database::Database,
    limit::Limit,
    offset::Offset,
    order::Ordering,
    physical_column::{FloatBits, IntBits, PhysicalColumn, PhysicalColumnType},
    physical_table::PhysicalTable,
    predicate::{CaseSensitivity, ParamEquality, Predicate},
    SQLBytes, SQLParam, SQLParamContainer,
};
