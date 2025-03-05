---
sidebar_position: 40
---

# exo schema

The `schema` subcommand allows you to work with the schema of your Exograph project.

## Creating a new schema

The `schema create` subcommand allows you to create a new schema file based on the current Postgres modules in your project. You invoke it from the project's root directory. By default, it will output the schema to stdout, but you can specify an output file using the `--output` (or the shorter `-o`) option.

```shell-session
# shell-command-next-line
exo schema create
```

```sql
CREATE TABLE "concerts" (
  "id" SERIAL PRIMARY KEY,
  "title" TEXT NOT NULL,
  "venue_id" INT NOT NULL,
  "published" BOOLEAN NOT NULL,
  "price" NUMERIC(20, 2) NOT NULL
);

...

```

## Verifying the existing schema

The `schema verify` subcommand allows you to check if the schema of your Exograph project is appropriate for your current project. It will point out any issues that it finds. This command requires either setting the `EXO_POSTGRES_URL` environment variable to the database URL you want to verify against or passing the `--database` (or the shorter `-d`) option with the database URL.

```shell-session
# shell-command-next-line
exo schema verify
This model is not compatible with the current database schema. You may need to update your model to match or perform a migration to update it.
The following issues should be corrected:
- The column `latitude` in the table `venues` exists in the model, but does not exist in the database table.
- The column `venueid` in the table `concerts` exists in the model, but does not exist in the database table.
- The column `price` in the table `concerts` exists in the model, but does not exist in the database table.

Error: Incompatible model.
```

When you run `exo yolo` or `exo dev`, Exograph will automatically verify your project's schema with every change.

One way to fix any issues that `schema verify` finds is to perform a migration.

## Migrating the schema

The `schema migrate` subcommand allows you to migrate the schema of your Exograph project. The migration file produced will have any destructive changes commented out (unless you pass the `--allow-destructive-changes` flag). Therefore, you should examine the migration file and deal with them appropriately. For example, when you rename a column, the migration file will mark (commented out) the deletion of the column with the old name and the addition of the column with the new name. Therefore, if renaming a field was your intention, you should replace those two with a "RENAME COLUMN" statement.

Like the `schema verify` command, this command requires either setting the `EXO_POSTGRES_URL` environment variable to the database URL you want to migrate against or passing the `--database` (or the shorter `-d`) option with the database URL.

```shell-session
# shell-command-next-line
exo schema migrate
```

```sql
ALTER TABLE "venues" ADD "latitude" REAL NOT NULL;
-- ALTER TABLE "concerts" DROP COLUMN "venue_id";
ALTER TABLE "concerts" ADD "venueid" INT NOT NULL;
ALTER TABLE "concerts" ADD "price" NUMERIC(20, 2) NOT NULL;
CREATE INDEX ON "venues" (latitude);
ALTER TABLE "concerts" ADD CONSTRAINT "concerts_venueid_fk" FOREIGN KEY ("venueid") REFERENCES "venues";
CREATE INDEX ON "concerts" (venueid);
CREATE INDEX ON "concerts" (price);
```

The `exo schema migrate` command offers a couple of options:

- The `--allow-destructive-changes` will not comment out destructive changes. If you are sure that you want to perform those changes, you can use this option.
- The `--apply-to-database` will apply changes to the database. This option is useful when applying the changes without running a separate `psql` command.

## Specifying the scope of the schema

By default, the `schema` subcommand operates on all tables of schemas used in your project (specified using either `@postgres(schema="...")` or `@table(schema="...")` in your exo files). This works well for brownfield projects where you want to create a new Exograph project that works with an existing database but skip migrating other schemas (which would suggest deleting any tables not referenced in your exo files). 

The default behavior works for most projects. However, you can specify a different migration scope using the `--scope` command line option, which is a comma-separated list in the form: `<schema-wildcard>[.<table-wildcard>]?` with `table-wildcard` being set to `*` by default. For example, to migrate only the `public` and `concerts` schemas, you can set the `--scope` option to `public.*,concerts.*`, or simply `public,concerts`. You may also specify a specific schema and table, for example, `public.concerts*` or even `*.concerts*`, etc.

# Creating an Exograph model from an existing database

:::warning
This feature is experimental and may not work as expected.
:::

The `schema import` subcommand allows you to create a new schema file based on the current Postgres database. This is useful when creating a new Exograph project from an existing database. You should examine the generated exo file, especially regarding access control rules.
