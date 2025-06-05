---
sidebar_position: 0
---

# Overview

The `schema` subcommand allows you to work with the schema of your Exograph project. It allows you to:

- [Create a schema for the current Exograph model](./create.md)
- [Verify the schema of your Exograph project](./verify.md)
- [Migrate the schema of your Exograph project](./migrate.md)
- [Import a model from an existing database](./import.md)

By default, the `schema` subcommand operates on all tables of schemas used in your project (specified using either `@postgres(schema="...")` or `@table(schema="...")` in your exo files). This works well for brownfield projects where you want to create a new Exograph project that works with an existing database but skip migrating other schemas (which would suggest deleting any tables not referenced in your exo files). 

However, you can specify a different migration scope using the `--scope` command line option, which is a comma-separated list in the form: `<schema-wildcard>[.<table-wildcard>]?` with `table-wildcard` being set to `*` by default. For example, to migrate only the `public` and `concerts` schemas, you can set the `--scope` option to `public.*,concerts.*`, or simply `public,concerts`. You may also specify a specific schema and table, for example, `public.concerts*` or even `*.concerts*`, etc.


