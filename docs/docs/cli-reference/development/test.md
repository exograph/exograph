---
sidebar_position: 60
---

# exo test

The `test` command runs the integration tests you wrote for your Exograph application. It takes the root directory of the tests (defaults to the current directory) and an optional argument for filtering which tests to run.

Like the [yolo] mode, it will use the locally installed Postgres server or start a Docker container as a fallback. During an `exo test` run, it will create a new database for each test and drop it after completion.

```shell-session
# shell-command-next-line
exo test <directory> [pattern]
```

You can control the database server that is used for testing by setting the `EXO_SQL_EPHEMERAL_DATABASE_LAUNCH_PREFERENCE` environment variable to one of the following values:
- `prefer-local`: Use the locally installed Postgres server if available, otherwise use a Docker container.
- `prefer-docker`: Use a Docker container if available, otherwise use the locally installed Postgres server.
- `local-only`: Only use the locally installed Postgres server.
- `docker-only`: Only use a Docker container.

When using a Docker container, you can override the default Docker image (currently `pgvector/pgvector:pg14`) that is used by setting the `EXO_SQL_EPHEMERAL_DATABASE_DOCKER_IMAGE` environment variable.

Please see the [testing](/production/testing.md) section for more information about writing tests.
