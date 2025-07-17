---
title: exo-server
slug: /cli-reference/production
---

# exo-server

While `exo dev` command is appropriate for development, it is unsuitable for production since it does more work than necessary, such as watching files for changes and verifying the schema. For production, you should use the `exo build` command to generate a production build of your schema and then use the `exo-server` to serve it.

Exograph support for production usage consists of two phases: building and serving. The building phase parses the input .exo files, checks the code for correctness, and generates a model (in a binary format). The serve phase reads the model and starts a GraphQL server.

## Building the exo_ir

You build the exo_ir using the `exo build` command. See [`exo build`](/cli-reference/development/build.md) for more information.

## Serving the exo_ir

You serve the exo_ir using the `exo-server` command. The command expects `target/index_exo_ir` as the location of the exo_ir file (which is the file created by the `exo build` command).

```shell-session
# shell-command-next-line
exo-server
```

It will print the information necessary to connect to the server:

```shell-session
Started server on 0.0.0.0:9876 in 5.47 ms
- GraphQL endpoint hosted at:
        http://0.0.0.0:9876/graphql
- MCP endpoint hosted at:
        http://0.0.0.0:9876/mcp
```

You can now send GraphQL queries to the endpoint using a GraphQL client such as [Apollo Client](https://www.apollographql.com/docs/react/) or [urql](https://formidable.com/open-source/urql/).

## Playground

What if you want to use the GraphQL playground with the production deployment with all the goodies, such as autocomplete, schema documentation, query history, and [integrated authentication](/authentication/overview.md)?

By default, the `exo-server` command will have its introspection and playground disabled. While you can enable it by setting the `EXO_INTROSPECTION` environment variable to `true`, it is not a [good idea to do so in production](/production/introspection.md). Instead, you can use the `exo playground` command that fetches the schema from your local model and executes GraphQL operations against the specified endpoint.

```shell-session
# shell-command-next-line
exo playground --endpoint http://0.0.0.0:9876/graphql -p 9999
```

It will print the information necessary to connect to the playground:

```shell-session
Starting playground server connected to the endpoint at: http://0.0.0.0:9876/graphql
- Playground hosted at:
        http://localhost:9999/playground
```

We pass the `-p` argument to specify the port to serve the playground on to avoid port conflict (the `exe-server` has already used the default 9876 port). In real-world production scenarios, you would not need to do this since you would be deploying it to a separate server, such as on [Fly.io](/deployment/flyio.md) or [AWS Lambda](/deployment/aws-lambda.md), so there is no port conflict with the local playground server.

You can now use the playground to interact with the GraphQL server. See [playground](/cli-reference/development/playground.md) for more information.
