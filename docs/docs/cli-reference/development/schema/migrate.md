---
sidebar_position: 30
title: exo schema migrate
---


# Migrating the schema

The `schema migrate` subcommand allows you to migrate the schema of your Exograph project. 

By default, the migration process will be in interactive mode, which will help with changes such as renaming a table, which would otherwise result in dropping the table and creating a new one. You can override this behavior by passing the `--non-interactive` flag.

The migration file produced will have any destructive changes commented out (unless you pass the `--allow-destructive-changes` flag). Therefore, you should examine the migration file and deal with these changes appropriately. For example, when you rename a column, the migration file will mark (commented out) the deletion of the column with the old name and the addition of the column with the new name. Therefore, if renaming a field was your intention, you should replace those two with a "RENAME COLUMN" statement.

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

The `exo schema migrate` command offers a few options:

- The `--allow-destructive-changes` will not comment out destructive changes. If you are sure that you want to perform those changes, you can use this option.
- The `--apply-to-database` will apply changes to the database. This option is useful when you want to apply the changes without running a separate `psql` command.
- The `--non-interactive` will not ask for any interactions. This is useful when you want to perform migration without any interactions.
- The `--interactions` allows you to specify a file with interactions for the migration, which is a toml file with the following structure:

```toml
[[rename-table]]
old-table = "todos"
new-table = "t_todos"

[[rename-table]]
old-table = "users"
new-table = "t_users"
```

In interactive mode, the `exo schema migrate` will use the interactions as needed and ask for any other changes.

In the future, we will support more interactions, such as renaming a column, adding a column, etc.

