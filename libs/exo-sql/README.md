# exo-sql

A library to interact with the SQL database in a simpler manner.

Note: Although a sub-project of Exograph, this should ultimately be a standalone
crate that can be used in other projects.

## Core Concepts

The core idea in this library is that of `AbstractOperation`, which along with
its variants, allows declaring an intention of a database operation at a higher
level. It also offers `DatabaseBackend` (with `PgBackend` as the Postgres
implementation), which is responsible for transforming an `AbstractOperation`
into one or more SQL operations and executing them. This separation of intention
vs execution allows for simplified expression from the user of the library and
leaves out the details of the database operations.

For example, consider `AbstractSelect`. It allows expressing the intention to
query data by specifying the root table, a predicate, and (potentially nested)
columns (among other things). It doesn't, however, express how to execute
the query; specifically, it doesn't specify any joins to be performed.
Similarly, `AbstractInsert` expresses an intention to insert logical rows
(columns into the root table as well as any referenced tables), but doesn't
specify how to go about doing so.

To allow expressing complex operations such as predicates based on nested
elements, the library requires the use of `ColumnPath`s in the predicates and
order by expressions. A `ColumnPath` is a path from the root table of the
operation to the intended column. Similarly, to allow inserting nested elements,
the library requires expressing the columns to be inserted as
`InsertionElement`s, which abstracts over columns and nested elements.

## Sub-crates

- `exo-sql-core`: Generic data model types (physical schema representation)
- `exo-sql-model`: Abstract SQL operations and transform traits
- `exo-sql-pg`: Postgres SQL types, generation, and transformation. Re-exports types from `exo-sql-core` and `exo-sql-model`.
- `exo-sql-pg-connect`: Postgres connection management and execution
- `exo-sql-pg-schema`: Postgres schema introspection, diff, and migration

Consumers should depend on `exo-sql-pg` for types and SQL generation, and
`exo-sql-pg-connect` only when connection management or execution is needed.
