You can control the database server that is used for testing by setting the `EXO_SQL_EPHEMERAL_DATABASE_LAUNCH_PREFERENCE` environment variable to one of the following values:

- `prefer-local` (default): Use the locally installed Postgres server if available; otherwise, use a Docker container.
- `prefer-docker`: Use a Docker container if Docker is installed; otherwise, use the locally installed Postgres server.
- `local-only`: Only use the locally installed Postgres server; otherwise, fail.
- `docker-only`: Only use a Docker container; otherwise, fail.
- `existing-db-only`: Use an already-running Postgres instance. This mode does not fall back to another option. If the existing Postgres is not reachable, the test fails. Optionally set `EXO_SQL_EXISTING_DB_URL` (e.g. `postgres://user@localhost:5432`) to specify connection details; defaults to the current OS user via the default Unix socket.

:::note
The `existing-db-only` mode only uses `createdb`/`dropdb`, skipping `initdb` and `pg_ctl` entirely. This makes it useful in sandboxed environments (e.g. Claude Code) where `initdb` fails due to restricted syscalls like `shmget`.
:::

When using a Docker container (either due to the launch preference or because the locally installed Postgres server is not available), you can override the default Docker image (currently `pgvector/pgvector:pg18`) by setting the `EXO_SQL_EPHEMERAL_DATABASE_DOCKER_IMAGE` environment variable.
