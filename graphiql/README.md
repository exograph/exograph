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
2. Comment out `window.exoGraphQLEndpoint` and `window.enableSchemaLiveUpdate`
3. Add `window.exoGraphQLEndpoint = "http://localhost:9876/graphql";`

Then, from the `graphiql` directory, run:

```
npm run start
```
