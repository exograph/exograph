---
sidebar_position: 10
title: exo schema create
---

# Creating a new schema

The `schema create` subcommand allows you to create a new schema file based on the Postgres modules in your current project. Invoke it from the project's root directory. By default, it will output the schema to stdout, but you can specify an output file using the `--output` (or the shorter `-o`) option.

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