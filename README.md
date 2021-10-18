# Development

## Prerequisites

Must have:

- Rust (see the version specified in [rust-toolchain.toml](rust-toolchain.toml))
- Postgres 12
- Treesitter (`cargo install --rev v0.19.5 --git https://github.com/tree-sitter/tree-sitter.git tree-sitter-cli`)

Nice to have:

- cargo-watch (`cargo install cargo-watch`)

## Installing the vscode extension

From the project root directory,

```
(cd $PWD/vscode-extension; npm run build)
ln -s $PWD/vscode-extension/out $HOME/.vscode/extensions/clay.vscode
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

During development, it is nicer to use `cargo watch` and let compilation and restart happen automatically with any source changes.

```
CLAY_JWT_SECRET="abcd" CLAY_CORS_DOMAINS="*" CLAY_DATABASE_URL=postgresql://localhost:5432/concerts-db CLAY_DATABASE_USER=$USER cargo watch --clear -x "run --bin clay serve integration-tests/basic-model-no-auth/concerts.clay"
```

**Note**
If you change the treesitter grammar source file, `cargo watch` doesn't seem pick up the change, so you need to run the non-watch version.

5. Run unit and integration tests

```
CLAY_TEST_DATABASE_URL=postgresql://localhost:5432 CLAY_TEST_DATABASE_USER=$USER cargo test
```

6. Run blackbox integration tests

```
CLAY_USE_CARGO=1 CLAY_CONNECTION_POOL_SIZE=1 CLAY_CHECK_CONNECTION_ON_STARTUP=false CLAY_TEST_DATABASE_URL=postgresql://$USER@localhost:5432 cargo run --bin clay test integration-tests
```
