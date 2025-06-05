---
sidebar_position: 40
title: exo schema import
---

# Importing an existing database

## Creating index.exo

The `schema import` subcommand allows you to create a new schema file based on an existing Postgres database. This is useful when creating a new Exograph project where a database already exists.

Create a new Exograph project and run the following command to update the `src/index.exo` file.

```shell-session
# shell-command-next-line
exo schema import
```

You can specify the database to use using either the `EXO_POSTGRES_URL` or `DATABASE_URL` environment variable or the `--database` option.

```shell-session
# shell-command-next-line
exo schema import --database postgres://user:password@host:port/database
```

By default, the index.exo file will have all access control rules set to `@access(false)`. You can override this through the `--query-access` and `--mutation-access` options.

```shell-session
# shell-command-next-line
exo schema import --query-access true 
```

Here, we are allowing all queries, but no mutations.

You should examine the generated exo file, especially regarding access control rules.

## Creating fragments from the database

Consider a scenario where you have a database that is used by multiple applications and you want to create a new Exograph project that works with it **without** letting Exograph manage the database (for example, performing migrations, etc.).

In this case, you can pass the `--fragments` option to the `schema import` subcommand. By default, it will print the fragments to stdout, but you can specify an output file using the `--output` (or the shorter `-o`) option.

```shell-session
# shell-command-next-line
exo schema import --fragments --output generated/fragments.exo
```

This will create a new `generated/fragments.exo` file that contains fragments based on the database schema.

You can then use the generated fragments to create types for your Exograph project. See [Using fragments](../../../core-concept/type#fragments) for more information.

