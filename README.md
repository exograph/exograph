# Development

## Prerequisites

Must have:

- Rust (see the version specified in [rust-toolchain.toml](rust-toolchain.toml)). Also install the wasm32-wasm target (`rustup target add wasm32-wasi`).
- Postgres 12
- Tree-sitter (`cargo install --version 0.20.6 tree-sitter-cli`)
- [Deno](https://deno.land/)

Nice to have:

- cargo-watch (`cargo install cargo-watch`)

## Installing the vscode extension

From the project root directory,

```
(cd $PWD/vscode-extension; npm run build)
ln -s $PWD/vscode-extension/out $HOME/.vscode/extensions/clay.vscode
```

## Building

1. Build the `graphiql` app:

Anytime you make a change to the `graphiql` app, you need to run this command.

```
(cd graphiql && npm install && npm run prod-build)
```

2. Build the `clay` and `clay-server` binaries:

```
cargo build
```

To create a production build:

```
cargo build --release
```

## Testing the setup

1. Create a test database

```
createdb concerts-db
```

2. Generate a schema for the test model

```
cargo run --bin clay schema create integration-tests/basic-model-no-auth/concerts.clay
```

3. Create the schema in the database

```
psql concerts-db
```

and then paste the output of the `clay schema create` command.

4. Start the server

```
CLAY_JWT_SECRET="abcd" CLAY_CORS_DOMAINS="*" CLAY_POSTGRES_URL=postgresql://localhost:5432/concerts-db CLAY_POSTGRES_USER=$USER cargo run --bin clay serve integration-tests/basic-model-no-auth/concerts.clay
```

During development, it is nicer to use `cargo watch` and let compilation and restart happen automatically with any source changes. You may also set `CLAY_INTROSPECTION=true` to allow GraphQL introspection queries.

```
CLAY_JWT_SECRET="abcd" CLAY_CORS_DOMAINS="*" CLAY_POSTGRES_URL=postgresql://localhost:5432/concerts-db CLAY_POSTGRES_USER=$USER CLAY_INTROSPECTION=true cargo watch --clear -x "run --bin clay serve integration-tests/basic-model-no-auth/concerts.clay"
```

When introspection is on, an interactive page is served at `/playground` by default; this is adjustable through the environment variable `CLAY_PLAYGROUND_HTTP_PATH`. The GraphQL endpoint accepts requests at `/graphql` by default; this is also adjustable through the environment variable `CLAY_ENDPOINT_HTTP_PATH`.

**Note**
If you change the treesitter grammar source file, `cargo watch` doesn't seem to pick up the change, so you need to run the non-watch version.

5. Run unit and integration tests

```
CLAY_TEST_POSTGRES_URL=postgresql://localhost:5432 CLAY_TEST_POSTGRES_USER=$USER cargo test
```

6. Run blackbox integration tests

```
cargo build && CLAY_TEST_POSTGRES_URL=postgresql://$USER@localhost:5432 target/debug/clay test integration-tests
```

## Logging, telemetry and tracing

The code is instrumented using the [tracing](https://crates.io/crates/tracing) library and wwill output spans and events to any configured tracing subscriber. There are also two out of the box options available with `clay-server` which can be enabled by setting the `CLAY_TELEMETRY` environment variable. Current options are `bunyan` and `jaeger`. The `bunyan` option will output [bunyan](https://crates.io/crates/tracing-bunyan-formatter) formatted data to stdout. The `jaeger` option will send data to a local Jaeger server. See the [opentelemetry-jaeger](https://crates.io/crates/opentelemetry-jaeger) docs for how to run one using docker.

The `RUST_LOG` environment variable can be used to configure the level(s) of spans and events which are recorded. It defaults to `info` but can be set to other standard log levels such as `debug` (which will also show logging from libraries such as `tokio-postgres`). More [sophisticated settings](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/struct.EnvFilter.html) can also be used to tune the output for specific crates and modules.

As an example, if we wanted to run the integration tests at debug level with the Jaeger integration, we would prefix the command above with `RUST_LOG=debug CLAY_TELEMETRY=jaeger`.

By default, if `CLAY_TELEMETRY` is not set, Claytip will log warnings and errors to the console. Log levels for console logging can be set through the `CLAY_CONSOLE_LOG` variable. Log levels may be set per-module; by default, `tracing_actix_web` and `actix_server::worker` are both set to `warn` (even if `CLAY_CONSOLE_LOG` is set to be more verbose than `warn`). This may be overriden through the following directive:

`CLAY_CONSOLE_LOG="tracing_actix_web=info,actix_server::worker=info"`
