---
sidebar_position: 20
title: exo schema verify
---

# Verifying the existing schema

The `schema verify` subcommand allows you to check if the schema of your Exograph project is compatible with your current database. It will point out any issues that it finds. This command requires either setting the `EXO_POSTGRES_URL` or `DATABASE_URL` environment variable or passing the `--database` (or the shorter `-d`) option with the database URL.

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