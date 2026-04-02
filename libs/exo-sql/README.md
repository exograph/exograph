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
- `exo-sql-pg-testing`: Integration test framework and CLI runner

Consumers should depend on `exo-sql-pg` for types and SQL generation, and
`exo-sql-pg-connect` only when connection management or execution is needed.

## Integration Testing

The `exo-sql-pg-testing` crate provides a framework for testing the full exo-sql pipeline: abstract SQL parsing, transformation, SQL generation, and execution against Postgres.

### Running Tests

```sh
cargo run --bin exo-sql-test -- libs/exo-sql/integration-tests
```

With a glob pattern filter:

```sh
cargo run --bin exo-sql-test -- libs/exo-sql/integration-tests "*nested*"
```

### Test Fixture Layout

```
integration-tests/
  basic-model/
    schema.sql              # DDL (CREATE TABLE statements)
    init.sql                # Seed data (INSERT statements)
    tests/
      simple-select.sqltest
      nested-relation.sqltest
```

- `schema.sql` defines the database tables. Backend-specific overrides (`schema.pg.sql`) take precedence.
- `init*.sql` files are loaded in sorted order. Backend-specific overrides (`init.pg.sql`) take precedence.
- `.sqltest` files define queries and expected results. `.pg.sqltest` files run only against Postgres.

### Test File Format (`.sqltest`)

Test files use TOML format with `[query]` and `[expect]` sections:

```toml
[query]
statement = "select concerts.id, concerts.name from concerts where concerts.venue_id.name = $1"
params = ["Madison Square Garden"]

[expect]
unordered_paths = ["/"]
result = [
  { id = 1, name = "Concert1" },
  { id = 3, name = "Concert3" },
]
```

### Abstract SQL Syntax

Queries use standard SQL parsed by `sqlparser-rs`, with dot-notation column paths:

- `concerts.id` -- column on the root table
- `concerts.venue_id.name` -- follow the FK column `venue_id` to the related table, select `name`

Relation segments use the FK column name directly (e.g., `venue_id`, not `venue` or `venues`). This is unambiguous and handles multiple FKs to the same table (e.g., `main_venue_id` vs `alt_venue_id`).

### JSON Aggregate Selection

Use `json_object()` or `json_agg()` to select nested related data:

```toml
[query]
statement = "select concerts.id, json_object(concerts.venue_id.id, concerts.venue_id.name) as venue from concerts"

[expect]
result = [
  { id = 1, venue = { id = 1, name = "Madison Square Garden" } },
]
```

### Unordered Results

Use `unordered_paths` to compare arrays as sets at specific paths:

- `"/"` -- top-level result array is unordered
- Nested paths (e.g., `"/venues"`) are supported for nested arrays
