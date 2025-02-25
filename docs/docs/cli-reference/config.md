---
sidebar_position: 3
---

# Configuration

Exograph tooling can be configured using an `exo.toml` file in the root directory of your project, such as:

```toml
[exograph]
version = "0.11.1"

[build]
after-model-change = [
  "exo graphql schema --output ../web/.gen/graphql/schema.graphql", 
  "exo schema migrate --allow-destructive-changes -o migrations/current.sql"
]
```

You can express the content in an [alternative TOML format](https://toml.io/en/v1.0.0). For example, the above configuration can be expressed as:

```toml
exograph.version = "0.11.1"

build.after-model-change = [
  "exo graphql schema --output ../web/.gen/graphql/schema.graphql", 
  "exo schema migrate --allow-destructive-changes -o migrations/current.sql"
]
```

The configuration file supports the `exograph` and `build`, `dev`, and `yolo` tables.

## `exograph`

The `exograph` table supports the `version` key to specify the required version of the Exograph CLI. 

A typical `exograph` table looks like this:

```toml
[exograph]
version = "0.11.1"
```

You can specify the version using as much specificity as you want. Here are some examples (along with the accepted Exograph CLI versions):

| Specification                 | Description                           |
|------------------------------|---------------------------------------|
| `version = "0.11.1"`         | Higher than 0.11.1, lower than 0.12.0 |
| `version = "1.2.3"`          | Higher than 1.2.3, lower than 2.0.0 |
| `version = "1.2"`            | Higher than 1.2.0, lower than 1.3.0 |
| `version = "1"`              | Higher than 1.0.0, lower than 2.0.0 |
| `version = ">=0.11.1"`       | Higher than or equal to 0.11.1 |
| `version = "<0.11.1"`        | Lower than 0.11.1 |
| `version = "=0.11.1"`        | Exactly 0.11.1 |

:::note Differences from npm/yarn/pnpm
Exograph enforces version constraints in the same way as [Rust's build tool, cargo, does](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#version-specifiers). For the most part, it matches the way npm/yarn/pnpm. However, there is a notable exception: By default, Exograph treats `1.2.3` the same way as npm would treat `~1.2.3`. Therefore, while `1.2.3` in a package.json would match only `1.2.3`, in an `exo.toml` file, it would match any semantically compatible version (>= `1.2.0` and < `1.3.0`).

You may want to explicitly specify the `~` or `=` prefixes to get the exact behavior of npm/yarn/pnpm.
:::

:::tip Recommendation
Until Exograph reaches version 1.0, we recommend the exact version specification (e.g., `version = "=0.11.1"`) or the minimal version with a patch version (e.g., `version = "~0.11.1"`).
:::

## `build`, `dev`, and `yolo`

The `build`, `dev`, and `yolo` tables support the `after-model-change` key, which specifies commands to run when the model changes.

When the specified commands run as follows:

- `build`: The commands run after the model has been built (such as directly building using `exo build` or indirectly building as part of `exo dev` or `exo yolo`).
- `dev`: The commands run when the server is restarted in the dev mode.
- `yolo`: The commands run when the server is restarted in the yolo mode.

For example, if you want to generate the GraphQL schema upon model changes automatically, you can use the following configuration:

```toml
[build]
after-model-change = [
  "exo graphql schema --output ../web/.gen/graphql/schema.graphql", 
]
```

During development, you may want to keep track of migrations that you will eventually need to apply to your staging or production database. You can do this by adding the following configuration:

```toml
[dev]
after-model-change = [
  "EXO_POSTGRES_URL=<staging-or-production-database-url> exo schema migrate --allow-destructive-changes -o migrations/current.sql"
]
```
