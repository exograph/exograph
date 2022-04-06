# Development

## Prerequisites

Must have:

- Rust (see the version specified in [rust-toolchain.toml](rust-toolchain.toml))
- Postgres 12
- Tree-sitter (`cargo install --rev v0.20.6 --git https://github.com/tree-sitter/tree-sitter.git tree-sitter-cli`)
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
CLAY_JWT_SECRET="abcd" CLAY_CORS_DOMAINS="*" CLAY_DATABASE_URL=postgresql://localhost:5432/concerts-db CLAY_DATABASE_USER=$USER cargo run --bin clay serve integration-tests/basic-model-no-auth/concerts.clay
```

During development, it is nicer to use `cargo watch` and let compilation and restart happen automatically with any source changes. You may also set `CLAY_INTROSPECTION=1` to allow GraphQL introspection queries.

```
CLAY_JWT_SECRET="abcd" CLAY_CORS_DOMAINS="*" CLAY_DATABASE_URL=postgresql://localhost:5432/concerts-db CLAY_DATABASE_USER=$USER CLAY_INTROSPECTION=1 cargo watch --clear -x "run --bin clay serve integration-tests/basic-model-no-auth/concerts.clay"
```

**Note**
If you change the treesitter grammar source file, `cargo watch` doesn't seem to pick up the change, so you need to run the non-watch version.

5. Run unit and integration tests

```
CLAY_TEST_DATABASE_URL=postgresql://localhost:5432 CLAY_TEST_DATABASE_USER=$USER cargo test
```

6. Run blackbox integration tests

```
cargo build && CLAY_TEST_DATABASE_URL=postgresql://$USER@localhost:5432 target/debug/clay test integration-tests
```
