---
sidebar_position: 10
---

# exo new

The `exo new` command creates a new skeleton Exograph project in the specified directory.

## Create a basic project

```shell-session
# shell-command-next-line
exo new <directory>
```

The `exo new` command takes one argument: the directory in which to create the project. If the directory does not exist, it will create it.

## Create a new project from an existing database

If you already have a database, you can create a new Exograph project from it by passing the `--from-database` flag. This command introspects the database and creates a new index.exo file based on the schema.

You can specify the database to use using either the `EXO_POSTGRES_URL` or `DATABASE_URL` environment variable or the `--database` option.

```shell-session
# shell-command-next-line
exo new <directory> --from-database --database postgres://user:password@host:port/database
```

By default, the index.exo file will have all access control rules set to `@access(false)`. You can override this through the `--query-access` and `--mutation-access` options. For example, to allow all queries, but no mutations (which is the default):

```shell-session
# shell-command-next-line
exo new <directory> --from-database --query-access true
```

You should examine the generated exo file, especially regarding access control rules.