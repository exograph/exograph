Since we didn't set the `EXO_INTROSPECTION` environment variable, it will be [`false` by default](/cli-reference/environment.md#control), which is a [good practice in production](/production/introspection.md). This default makes the GraphQL playground unavailable with the production URL. Thanks to `exo playground` command, this is not a problem. Run the following command to open the playground:

```shell-session
# shell-command-next-line
exo playground --endpoint https://<server-url>/graphql
```

This will print a URL to the playground. Open it in your browser. You should see the playground.

```shell-session
Starting playground server connected to the endpoint at: https://<server-url>/graphql
- Playground hosted at:
        http://localhost:9876/playground
```

Now open http://localhost:9876/playground to see the GraphiQL Playground. You can execute queries and mutations. See the [Getting Started](/getting-started/local.md#using-the-graphiql-interface) guide for more details.
