<a href="https://exograph.dev">
  <p align="center">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="logo-dark.svg">
      <source media="(prefers-color-scheme: light)" srcset="logo-light.svg">
      <img alt="Exograph" src="logo-light.svg">
    </picture>
  </p>
</a>

[Exograph](https://exograph.dev) is a declarative way to create flexible, secure, and performant backends that provide GraphQL query and mutation APIs. Exograph lets you focus on your domain model and business logic, freeing you to pursue more creative work on your application. Furthermore, it offers tooling to support all stages of the development lifecycle, from development to deployment to maintenance.

# Installation

Get started by following the [Getting Started](https://exograph.dev/docs/getting-started) guide.

# Development

## Prerequisites

Must have:

- [Rustup](https://rustup.rs/).
- [Protobuf-compiler version 3.x](https://grpc.io/docs/protoc-installation/)
- [Postgres 12 or above](https://www.postgresql.org/)
- [Node 16 or above](https://nodejs.org/en)

Nice to have:

- cargo-watch (`cargo install cargo-watch`)

## Installing the vscode extension

Follow the instructions in [Exograph VSCode Extension repo](https://github.com/exograph/vscode-extension).

## Building

Build the `exo` and `exo-server` binaries:

```
cargo build
```

To create a production build:

```
cargo build --release
```

By default, cargo will build the `exo-server` binary with statically linked plugins. If you want to build a binary that dynamically links these plugins, you can use the `--no-default-features` flag:

```
cargo build --no-default-features
```

You can also selectively enable static linking for either Postgres or Deno:

```
cargo build --no-default-features --features static-postgres-resolver
cargo build --no-default-features --features static-deno-resolver
cargo build --no-default-features --features static-wasm-resolver
```

## Testing the setup

### Yolo mode

```sh
cd integration-tests/basic-model-no-auth
cargo run --bin exo yolo
```

### Dev mode

1. Switch to a Exograph project directory (e.g. `integration-tests/basic-model-no-auth`)

```sh
cd integration-tests/basic-model-no-auth
```

2. Create a test database

```sh
createdb concerts-db
```

3. Update the schema

```sh
cargo run --bin exo schema create | psql concerts-db
```

4. Start the server

```sh
EXO_JWT_SECRET="abcd" EXO_CORS_DOMAINS="*" EXO_POSTGRES_URL=postgresql://localhost:5432/concerts-db EXO_POSTGRES_USER=$USER cargo run --bin exo dev
```

During development, it is nicer to use `cargo watch` and let compilation and restart happen automatically with any source changes. You may also set `EXO_INTROSPECTION=true` to allow GraphQL introspection queries.

```sh
EXO_JWT_SECRET="abcd" EXO_CORS_DOMAINS="*" EXO_POSTGRES_URL=postgresql://localhost:5432/concerts-db EXO_POSTGRES_USER=$USER EXO_INTROSPECTION=true cargo watch -cx "run --bin exo dev"
```

When introspection is on, an interactive page is served at `/playground` by default; this is adjustable through the environment variable `EXO_PLAYGROUND_HTTP_PATH`. The GraphQL endpoint accepts requests at `/graphql` by default; this is also adjustable through the environment variable `EXO_ENDPOINT_HTTP_PATH`.

**Note**
If you change the tree-sitter grammar source file, `cargo watch` doesn't seem to pick up the change, so you need to run the non-watch version.

5. Run unit tests

```sh
cargo build && cargo test
```

6. Run integration tests

```sh
cargo build && EXO_RUN_INTROSPECTION_TESTS=true cargo run --bin exo test integration-tests
```

## Logging, telemetry and tracing

The code is instrumented using the [tracing](https://crates.io/crates/tracing) framework and will output log events to the console by default. The log level can be configured by setting the `EXO_LOG` variable which behaves identically to `RUST_LOG`. It defaults to `info` but can be set to other standard log levels such as `debug` (which will also show logging from libraries such as `tokio-postgres`). More [sophisticated settings](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/struct.EnvFilter.html) can also be used to tune the output for specific crates and modules.

For more details, including how to set up OpenTelemetry, see the [Exograph telemetry documentation](https://exograph.dev/docs/production/telemetry).
