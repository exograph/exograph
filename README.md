<a href="https://exograph.dev">
  <p align="center">
    <picture width=80%>
      <source media="(prefers-color-scheme: dark)" srcset="logo-dark.png">
      <source media="(prefers-color-scheme: light)" srcset="logo-light.png">
      <img alt="Exograph" src="logo-light.svg">
    </picture>
  </p>
</a>

<div align="center">

[![X badge][]][X link]
[![Discord badge][]][Discord link]

</div>

<p align="center">
  <a href="https://exograph.dev/docs/getting-started">Getting Started</a> •
  <a href="https://exograph.dev/docs">Documentation</a> •
  <a href="https://github.com/exograph/examples">Examples</a>
</p>

<br/>

[Exograph](https://exograph.dev) is a declarative way to create flexible, secure, and performant backends that provide GraphQL query and mutation APIs. Exograph lets you focus on your domain model and business logic, freeing you to pursue more creative work on your application. Furthermore, it offers tooling to support all stages of the development lifecycle, from development to deployment to maintenance.

# Installation

Get started by following the [Getting Started](https://exograph.dev/docs/getting-started) guide.

# Development

## Prerequisites

Must have:

- [Rustup](https://rustup.rs/)
- [Protobuf-compiler version 3.x](https://grpc.io/docs/protoc-installation/)
- [Postgres 12 or above](https://www.postgresql.org/)
- [Node 16 or above](https://nodejs.org/en)

Nice to have:

- cargo-watch (`cargo install cargo-watch`)

## Installing the vscode extension

Follow the instructions in [Exograph VSCode Extension repo](https://github.com/exograph/vscode-extension).

## Building

Build the `exo` and `exo-server` binaries:

```sh
cargo build
```

To create a production build:

```sh
cargo build --release
```

By default, cargo will build the `exo-server` binary with statically linked plugins. If you want to build a binary that dynamically links these plugins, you can use the `--no-default-features` flag:

```sh
cargo build --no-default-features
```

You can also selectively enable static linking for either Postgres or Deno:

```sh
cargo build --no-default-features --features static-postgres-resolver
cargo build --no-default-features --features static-deno-resolver
cargo build --no-default-features --features static-wasm-resolver
```

## Running tests

### Unit tests

```sh
cargo build && cargo test
```

### Integration tests

```sh
cargo build && EXO_RUN_INTROSPECTION_TESTS=true cargo run --bin exo test integration-tests
```

## Testing the setup

### Yolo mode

```sh
cd integration-tests/basic-model-no-auth
cargo run --bin exo yolo
```

You will see URLs for the GraphQL playground and GraphQL endpoint. You can use the playground to run queries and mutations against the endpoint.

### Dev mode

1. Switch to an example Exograph project directory (such as `integration-tests/basic-model-no-auth`)

```sh
cd integration-tests/basic-model-no-auth
```

2. Create a test database and update its schema

```sh
createdb concerts-db
cargo run --bin exo schema create | psql concerts-db
```

3. Start the server

```sh
EXO_JWT_SECRET="abcd" EXO_POSTGRES_URL=postgresql://localhost:5432/concerts-db EXO_POSTGRES_USER=$USER cargo run --bin exo dev
```

During development, it is nicer to use `cargo watch` and let compilation and restart the server automatically with any source changes.

```sh
EXO_JWT_SECRET="abcd" EXO_POSTGRES_URL=postgresql://localhost:5432/concerts-db EXO_POSTGRES_USER=$USER cargo watch -cx "run --bin exo dev"
```

Please see [CLI Reference](https://exograph.dev/docs/cli-reference/environment) for options such as setting paths for the GraphQL playground and query endpoint.

## Logging, tracing, and telemetry

The code is instrumented using the [tracing](https://crates.io/crates/tracing) framework and will output log events to the console by default. For more details, including setting logging levels and using OpenTelemetry, see the [Exograph telemetry documentation](https://exograph.dev/docs/production/telemetry).

[X badge]: https://img.shields.io/twitter/follow/ExographDev
[X link]: https://twitter.com/ExographDev
[Discord badge]: https://img.shields.io/discord/1099019056624975993?logo=discord&style=social
[Discord link]: https://discord.gg/eeWYx9NtMW
