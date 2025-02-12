---
sidebar_position: 50
---

# exo graphql

The `graphql` subcommand allows you to work with the GraphQL of your Exograph project.

## Generating a new GraphQL schema

The `graphql schema` subcommand creates a file with the GraphQL schema to use with tools like [codegen](https://the-guild.dev/graphql/codegen). You invoke it from the project's root directory.

```shell-session
# shell-command-next-line
exo graphql schema
```

The command takes two optional arguments:

- `format`: The format of the output file. It can be `json` or `graphql`. The default is `graphql`.
- `output`: The path to the output file. The default is `generated/schema.graphql` if the format is `graphql`, and `generated/schema.json` if the format is `json`.

For example, to generate a JSON schema and save it to `my-schema.json`, you can run:

```shell-session
# shell-command-next-line
exo graphql schema --format json --output my-schema.json
```

To generate a GraphQL schema and save it to `my-schema.graphql`, you can run:
```shell-session
# shell-command-next-line
exo graphql schema --format graphql --output my-schema.graphql
```
