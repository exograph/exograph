---
sidebar_position: 30
---

# exo dev

While you may start with the ["yolo" mode](yolo.md), you will eventually want to persist data between runs. Therefore, Exograph offers the "dev" mode with a smooth development experience while offering sufficient control. In this mode, Exograph watches for changes to project sources, verifies that the database schema is appropriate, and applies migrations.

It requires that you create a database and set a few environment variables. Unlike the "yolo" mode, it will not delete the database when you stop the server. Therefore, you can continue with the data from one run to the next.

## Usage

You invoke the dev mode using the `dev` command. Like the `yolo` command, you invoke it in the project's root directory.

```shell-session
# shell-command-next-line
exo dev
```

If your project uses a Postgres module, you must create a database with the appropriate schema. Please see the [exo schema](schema/create.md) command for more information. You will also need to export the `EXO_POSTGRES_URL` environment variable to that database (there are a few other options. Please see the [configuration](/postgres/configuration.md) page for more details).

By default, it will start the server on port 9876. You can change this by passing the `--port` (or the shorter `-p`) option.

By default, it will enforce [trusted documents](../../production/trusted-documents.md). You can turn this off by passing the `--enforce-trusted-documents=false` option.

By default, it will apply migrations and if a migration fails, it will pause to allow you to fix the problem. You can turn this off by passing the `--ignore-migration-errors` option.

```shell-session
# shell-command-next-line
exo dev --port 8888
```

It will print the information necessary to connect to the server.

```shell-session
Starting server in development mode...
Watching the src directory for changes...

Verifying new model...
Started server on localhost:9876 in 2.61 ms
- GraphQL hosted at:
        http://localhost:9876/graphql
- MCP endpoint hosted at:
        http://localhost:9876/mcp        
- Playground hosted at:
        http://localhost:9876/playground
```

Exograph will restart the server and apply the necessary migrations whenever you change any file in the directory. If the database schema doesn't match the expected schema, it will automatically apply safe migrations.

```shell-session
Verifying new model...
Applying migration...
Migration successful!
```

Auto-applying migration may fail. For example, consider if a table already has a few rows, and you change the model to add a new non-optional field. In this case, the migration will attempt to add a new column to the table, but the existing rows will make this migration fail. In such cases, or for any destructive migration, it will prompt you with a few options.

```shell-session
The schema of the current database is not compatible with the current model for the following reasons:
- The non-nullable column `info` in the table `venues` exists in the database table, but does not exist in the model.

? Choose an option:
> Attempt migration
> Continue with old schema
> Pause for manual repair
> Exit
```

If you choose "Attempt migration", it will confirm about applying destructive changes.

If you continue without fixing it, it will restart the server without applying the migration, which may result in errors later.
