# GraphiQL interface to Exograph

An app with a few customizations around the [`GraphiQL` component](https://github.com/graphql/graphiql/tree/main/packages/graphiql).

# Building (automatically run by `build.rs`)

```
npm ci
npm run prod-build
```

# Development tip

When working solely on the UI aspect of the playground, you can get faster iteration times by running it as a standalone app. To do so, temporarily modify "index.html" as follows:

1. Comment out the existing `<base ...` and add `<base href="/" />`
2. Replace `window.exoConfig = {}` with the following:

```javascript
window.exoConfig = {
  playgroundHttpPath: "/playground",
  graphqlHttpPath: "http://localhost:9876/graphql",
  mcpHttpPath: "http://localhost:9876/mcp",
  enableSchemaLiveUpdate: false,
};
```

3. When exploring OIDC, add `exoOidcUrl = "<provider>";`

Then, from the `graphiql/lib` directory, run:

```
npm run build
```

And from the `graphiql/app` directory, run:

```
npm run dev
```
