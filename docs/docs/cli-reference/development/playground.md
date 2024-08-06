---
sidebar_position: 90
---

# exo playground

Following the best practice, you would have introspection turned off in production. However, this makes it harder to explore your APIs. The `playground` command helps you in this situation. This command starts a local server based on the model in the current directory (same as the `exo yolo` or `exo dev` command). It uses this local server to fetch the schema and serve the GraphQL playground. However, it executes GraphQL operations against the specified endpoint.

## Usage

```shell-session
# shell-command-next-line
exo playground --endpoint <the-endpoint-url>
```

You must pass the endpoint URL using the `--endpoint` option (typically, it will look like `https://<production-url>/graphql`). You can also pass the `--port` option to specify the port to serve the playground. By default, it will use port 9876.

Either way, it will print the information necessary to connect to the playground.

```shell-session
Starting playground server connected to the endpoint at: <the-endpoint-url>
- Playground hosted at:
        http://localhost:9876/playground
```

:::note
This doesn't bypass the recommended practice of turning off introspection in production. The `exo playground` is useful only if you have access to the server's source code, in which case, you would know the schema, anyway!
:::

You will see a UI element in the playground showing the specified endpoint. Besides this difference, the playground will behave identically to the local playground, including autocomplete, schema documentation, query history, and [integrated authentication](/authentication/overview.md).

## Schema Sync

The playground mode uses schema based on the current directory where you invoke the command. You should ensure that the schema in the current directory matches the schema in the production environment. One way to do so is to tag the commit that you deploy to production (which is a good and common practice in any case) and then check out that tag locally before invoking the `playground` command.
