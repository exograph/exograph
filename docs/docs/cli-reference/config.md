---
sidebar_position: 3
---

# Configuration

Exograph developers tooling can be configured using an `exo.toml` file in the root directory of your project. Here is an example configuration file:

```toml
[tool-version]
version = "0.11.1"

[watch]
before = ["echo 'before1'", "echo 'before2'"]
after = [
  "exo graphql schema --output ../web/.gen/graphql/schema.graphql", 
  "exo schema migrate --allow-destructive-changes -o migrations/current.sql"
]
```

You can express the content in an [alternative TOML format](https://toml.io/en/v1.0.0), if you prefer. For example, the above configuration can be expressed as:

```toml
tool-version = "0.11.1"

watch.before = [
  "echo 'before1'",
  "echo 'before2'"
]

watch.after = [
  "exo graphql schema --output ../web/.gen/graphql/schema.graphql",
  "exo schema migrate --allow-destructive-changes -o migrations/current.sql"
]
```

The configuration file supports the `tool-version` and `watch` keys.

## `tool-version`

The `tool-version` specifies the required version of the Exograph CLI using as much specificity as you want. Here are some examples (along with the accepted Exograph CLI versions):

| Specification                     | Description                                           |
|-----------------------------------|-------------------------------------------------------|
| `tool-version = "0.11.1"`         | Higher than 0.11.1, lower than 0.12.0 |
| `tool-version = "1.2.3"`          | Higher than 1.2.3, lower than 2.0.0 |
| `tool-version = "1.2"`            | Higher than 1.2.0, lower than 1.3.0 |
| `tool-version = "1"`              | Higher than 1.0.0, lower than 2.0.0 |
| `tool-version = ">=0.11.1"`       | Higher than or equal to 0.11.1 |
| `tool-version = "<0.11.1"`        | Lower than 0.11.1 |
| `tool-version = "=0.11.1"`        | Exactly 0.11.1 |

:::tip Recommendation
Until Exograph reaches version 1.0, we recommend the exact version specification (e.g. `tool-version = "=0.11.1"`) or the minimal version with a patch version   (e.g. `tool-version = "0.11.1"`).
:::

## `watch`

The `watch` key is used to specify the commands to run when the configuration changes. The `before` key is a list of commands to run before building the model (through either the `exo build` command or as a part of the `exo dev` or `exo yolo` commands). The `after` key is a list of commands to run after the model has been built.
