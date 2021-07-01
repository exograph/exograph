# Development

## Prerequisites

Must have:

- Rust (the latest version)
- Postgres 12
- Treesitter (`npm install -g treesitter`)

Nice to have:

- cargo-watch (`cargo install cargo-watch`)

## Installing the vscode extension

From the project root directory,

```
ln -s $PWD/vscode-extension $HOME/.vscode/extensions/clay.vscode
```

## Testing the setup

1. Create a test database

```
createdb concerts-db
```

2. Generate a schema for the test model

```
cargo run --bin clay schema create integration-tests/basic-model-no-auth/concerts.payas
```

3. Create the schema in the database

```
psql concerts-db
```

and then paste the output of the `clay schema create` command.

4. Start the server

```
CLAY_JWT_SECRET="abcd" CLAY_CORS_DOMAINS="*" CLAY_DATABASE_URL=postgresql://localhost:5432/concerts-db CLAY_DATABASE_USER=$USERNAME cargo run --bin clay-server integration-tests/basic-model-no-auth/concerts.clay
```

During development, it is nicer to use `cargo watch` and let compilation and restart happen automatically with any source changes.

```
CLAY_JWT_SECRET="abcd" CLAY_CORS_DOMAINS="*" CLAY_DATABASE_URL=postgresql://localhost:5432/concerts-db CLAY_DATABASE_USER=$USERNAME cargo watch --clear -x "run --bin clay serve integration-tests/basic-model-no-auth/concerts.clay"
```

**Note**
If you change the treesitter grammar source file, `cargo watch` doesn't seem pick up the change, so you need to run the non-watch version.

5. Run the integration tests

```
CLAY_USE_CARGO=1 CLAY_TEST_DATABASE_URL=postgresql://ramnivas@localhost:5432 cargo run --bin clay-test integration-tests
```
