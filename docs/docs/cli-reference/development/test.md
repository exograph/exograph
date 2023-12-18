---
sidebar_position: 5
---

# exo test

The `test` command runs the integration tests you wrote for your Exograph application. It takes the root directory of the tests (defaults to the current directory) and an optional argument for filtering which tests to run.

Like the [yolo] mode, it will use the locally installed Postgres server or start a Docker container as a fallback. During an `exo test` run, it will create a new database for each test and drop it after completion.

```shell-session
# shell-command-next-line
exo test <directory> [pattern]
```

Please see the [testing](/production/testing.md) section for more information about writing tests.
