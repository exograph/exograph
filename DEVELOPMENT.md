This file describes how to build Exograph locally (for example, to contribute to the project).

# Prerequisites

Must have:

- [Rustup](https://rustup.rs/)
- [Protobuf-compiler version 3.x](https://grpc.io/docs/protoc-installation/)
- [Postgres 12 or above](https://www.postgresql.org/)
- [pgvector](https://github.com/pgvector/pgvector)
- [Node 16 or above](https://nodejs.org/en)

Nice to have:

- cargo-watch (`cargo install cargo-watch`)

# Installing the vscode extension

Follow the instructions in [Exograph VSCode Extension repo](https://github.com/exograph/vscode-extension).

# Building

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

# Running tests

## Unit tests

```sh
cargo build && cargo test
```

## Integration tests

```sh
cargo build && EXO_RUN_INTROSPECTION_TESTS=true cargo run --bin exo test integration-tests
```

# Testing the setup

## Yolo mode

```sh
cd integration-tests/basic-model-no-auth
cargo run --bin exo yolo
```

You will see URLs for the GraphQL playground and GraphQL endpoint. You can use the playground to run queries and mutations against the endpoint.

## Dev mode

1. Switch to an example Exograph project directory (such as `integration-tests/basic-model-no-auth`)

```sh
cd integration-tests/basic-model-no-auth
```

2. Create a test database

```sh
createdb concerts-db
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

# Logging, tracing, and telemetry

The code is instrumented using the [tracing](https://crates.io/crates/tracing) framework and will output log events to the console by default. For more details, including setting logging levels and using OpenTelemetry, see the [Exograph telemetry documentation](https://exograph.dev/docs/production/telemetry).
