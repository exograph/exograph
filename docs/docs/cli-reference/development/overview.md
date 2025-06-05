---
sidebar_position: 0
slug: /cli-reference/development
---

# Overview

To provide a better development experience, Exograph offers a development cli tool--`exo`. You can explore the commands using the `exo --help` command.

```shell-session
# shell-command-next-line
exo --help
Command line interface for Exograph

Usage: exo <COMMAND>

Commands:
  new     Create a new Exograph project
  yolo    Run local exograph server with a temporary database
  dev     Run exograph server in development mode
  build   Build exograph server binary
  deploy  Deploy your Exograph project
  schema  Create, migrate, verify, and import  database schema
  test    Perform integration tests

Options:
  -h, --help     Print help
  -V, --version  Print version
```

We will look at each of the commands in the following sections.

- [exo new](new.md)
- [exo yolo](yolo.md)
- [exo dev](dev.md)
- [exo build](build.md)
- [exo deploy](deploy.md)
- [exo schema](schema/overview.md)
- [exo test](test.md)
