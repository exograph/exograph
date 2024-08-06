---
sidebar_position: 50
---

# exo graphql

The `graphql` subcommand allows you to work with the GraphQL of your Exograph project.

## Generating a new GraphQL schema

The `graphql schema` subcommand creates a file with the GraphQL schema to use with tools like [codegen](https://the-guild.dev/graphql/codegen). You invoke it from the project's root directory. By default, it will output the schema to `generated/schema.json`, but you can specify an output file using the `--output` (or the shorter `-o`) option.

```shell-session
# shell-command-next-line
exo graphql schema
``
```
