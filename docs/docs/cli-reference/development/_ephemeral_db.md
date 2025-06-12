You can control the database server that is used for testing by setting the `EXO_SQL_EPHEMERAL_DATABASE_LAUNCH_PREFERENCE` environment variable to one of the following values:
- `prefer-local` (default): Use the locally installed Postgres server if available; otherwise, use a Docker container.
- `prefer-docker`: Use a Docker container if Docker is installed; otherwise, use the locally installed Postgres server.
- `local-only`: Only use the locally installed Postgres server; otherwise, fail.
- `docker-only`: Only use a Docker container; otherwise, fail.

When using a Docker container (either due to the launch preference or because the locally installed Postgres server is not available), you can override the default Docker image (currently `pgvector/pgvector:pg14`) by setting the `EXO_SQL_EPHEMERAL_DATABASE_DOCKER_IMAGE` environment variable.