---
sidebar_position: 2
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

# Limiting queries and mutations

Exograph (as any GraphQL server) offers a wide range of APIs to query and mutate data and allow any valid selection of fields. However, typical applications don't need all of this flexibility. So, it is a good idea to restrict the queries and mutations to only those used by client applications. Exograph provides a way to do this using trusted documents (also known as **persisted queries** or **persisted documents**).

<details>
<summary>We will use the term "executable document" as the unit of "trust" since it is clearer and less ambiguous than "query" or "mutation".</summary>

The term "query" is overloaded in the context of GraphQL: It can refer to either of the following:

- an individual query such as `todos`
- a collection of queries, such as
  ```graphql
  query allTodos {
    completed: todos(where: {completed: {eq: true}}) {
      ...
    }
    incomplete: todos(where: {completed: {eq: false}}) {
      ...
    }
  }
  ```
- a part of the payload sent by a client (which can be a query, mutation, or subscription).

```json
{
  "query": "query todos...",
  ...
}
```

or

```json
{
  "query": "mutation updateTodo...",
  ...
}
```

It is this last use that we are interested in applying trust. A better term for the value of the `query` field in the payload is "executable document", as defined [by the GraphQL spec](https://spec.graphql.org/draft/#ExecutableDocument).

</details>

## What are trusted documents?

Trusted documents are the executable documents that form the "allow" list for the server. The server allows only those executable documents and rejects all others.

For example, if an application uses _only_ the following query:

```graphql
query getTodos($completed: Boolean!) {
  todos(where: { completed: { eq: $completed } }) {
    id
    completed
    title
  }
}
```

And the following mutation:

```graphql
mutation updateTodo($id: Int!, $completed: Boolean!, $title: String!) {
  updateTodo(id: $id, data: { title: $title, completed: $completed }) {
    id
  }
}
```

With the trusted documents support, you can instruct the server to accept only these two and reject all others. It won't even allow the following (which selects fewer fields than the original):

```graphql
query getTodos($completed: Boolean!) {
  todos(where: { completed: { eq: $completed } }) {
    id
  }
}
```

Using trusted documents reduces the effective API surface area and offers several benefits, such as:

- Preventing attackers from executing arbitrary queries and mutations.
- Reducing the bandwidth by sending only the hash of the query instead of the query itself.
- Simplifying [testing](testing.md) by focusing on the actual queries and mutations used by the client application.
- Allowing server-side optimizations such as avoiding query parsing and pre-planning query execution.

For a more detailed discussion on the motivation behind trusted documents, including the reason to prefer the term "trusted documents", please see the [GraphQL Trusted Documents](https://benjie.dev/graphql/trusted-documents).

## Workflow for using trusted documents

Using trusted documents with Exograph is easy. While we will delve into the details of how to set up trusted documents [later](#example), the high-level workflow is as follows:

1. Have the client generate a list of trusted documents. Typically, a tool will help produce this list, which will examine the application for all the queries and mutations used by the client application.
2. Set up the server to accept only trusted documents. You would create a `trusted-documents` directory and add the files generated in the earlier step.
3. Set up the client to send only the hashes of the executable documents instead of the full text.

The server will now accept only the trusted documents.

## Supported formats

Exograph supports two formats to express trusted documents:

1. The format used by the [`graphql-codegen`](https://the-guild.dev/graphql/codegen/plugins/presets/preset-client#persisted-documents) tool (and can be used by the [`persistedExchange` with URQL](https://commerce.nearform.com/open-source/urql/docs/advanced/persistence-and-uploads/)):

```json
{
  "<hash>": "<document>",
  ...
}
```

2. The format used by Apollo's [generate-persisted-query-manifest](https://www.npmjs.com/package/@apollo/generate-persisted-query-manifest) tool (and can be used by the [`@apollo/persisted-query-lists`](https://www.npmjs.com/package/@apollo/persisted-query-lists) link):

```json
{
    "operations": [
      {
        "id": "<hash>",
        "body": "<document>"
      },
      ...
    ]
}
```

In either case, `<hash>` is the SHA-256 hash of the document, and the `<document>` is the executable document text.

:::note Automatic persisted queries
Automatic persistent queries ([APQ](https://www.apollographql.com/docs/apollo-server/performance/apq/)) allow saving bandwidth by sending a hash of the query instead of the query itself. With APQ, the client and the server use a protocol to negotiate the query and its hash. The server then looks up the query using the hash and executes it. However, it doesn't prevent the client from sending any executable document and thus doesn't offer any security benefits. For this reason, Exograph doesn't support automatic persisted queries.
:::

## Organizing trusted documents

To use trusted documents, you need to set up the server to accept only trusted documents by creating a directory named `trusted-documents` with the trusted documents with either form shown earlier.

An Exograph server may serve multiple clients, each with its own set of trusted documents. For example, a web client may have its own set of trusted documents, and an iOS client may have its own set of trusted documents. Exograph supports this scenario by allowing you to place any trusted documents files anywhere inside the `trusted-documents` directory.

For example, you may include one file per client. In the following example, the `web.json` file contains the trusted documents for the web client, the `ios.json` file contains the trusted documents for the iOS client, and so on.

```
todo-app
├── src
│ └── ...
├── trusted-documents
│ ├── web.json
│ └── ios.json
│ └── android.json
```

You may also organize the trusted document in subdirectories, which is useful when each client has multiple kinds or multiple versions of trusted documents.

```
todo-app
├── src
│ └── ...
├── trusted-documents
│ ├── web
│ │ └── user-facing.json
│ │ └── admin-facing.json
│ └── ios
│ │ └── core.json
│ │ └── admin.json
│ └── android
│ │ └── version1.json
│ │ └── version2.json
```

There may be an overlap of trusted documents between clients. For example, the `user-facing.json` and `admin-facing.json` trusted documents may have some common elements.

## Example

Let's walk through an example of how to set up trusted documents for a typical client application. We will consider two popular GraphQL client libraries: Apollo Client and URQL. See the complete application here: [Apollo version](https://github.com/exograph/examples/tree/main/todo-with-nextjs) and [URQL version](https://github.com/exograph/examples/tree/main/todo-with-nextjs-urql).

### Creating trusted documents

The first step is to create the trusted documents. This step involves examining the client application to find the queries and mutations used by the client application. This step often needs to be coordinated with the last step of [setting up the client](#setting-up-the-client). For example, it is best to use Apollo's `generate-persisted-query-manifest` tool to generate trusted documents for the Apollo Client and use the link provided by the `@apollo/persisted-query-lists` package to send only the hashes.

It is a good idea to automate the generation of trusted documents by, for example, adding it as a `predev` step if you are working with a JavaScript/TypeScript application.

<Tabs groupId="client-choice">
  <TabItem value="apollo" label="Apollo Client" default>

You must add `@apollo/generate-persisted-query-manifest` as a development dependency.

```sh
npm install --save-dev @apollo/generate-persisted-query-manifest
```

You can run the following command to generate the trusted documents.

```sh
npx generate-persisted-query-manifest
```

However, it is best to automate this step.

```json
{
  ...
  "scripts": {
    "predev": "... && npx generate-persisted-query-manifest && ...",
    ...
  },
  ...
}
```

If you are using `graphql-codegen` there is an [alternative](https://www.npmjs.com/package/@apollo/generate-persisted-query-manifest#usage-with-graphql-codegen-persisted-documents) that you may explore.

  </TabItem>
  <TabItem value="urls" label="URQL Client" default>

We will use the `@graphql-codegen` tool to generate the trusted documents. We need to configure the code generation to produce the trusted documents. The following is an example of the configuration file for the `@graphql-codegen` tool.

```ts
import type { CodegenConfig } from "@graphql-codegen/cli";
import { addTypenameSelectionDocumentTransform } from "@graphql-codegen/client-preset";

const config: CodegenConfig = {
  schema: "http://localhost:9876/graphql",
  documents: ["src/app/**/*.{ts,tsx}", "src/components/**/*.{ts,tsx}"],
  generates: {
    "./src/__generated__/": {
      preset: "client",
      presetConfig: {
        extension: ".generated.tsx",
        baseTypesPath: "types.ts",
        fragmentMasking: { unmaskFunctionName: "getFragmentData" },
        // highlight-start
        persistedDocuments: {
          hashAlgorithm: "sha256",
        },
        // highlight-end
      },
      // highlight-next-line
      documentTransforms: [addTypenameSelectionDocumentTransform],
      plugins: [],
    },
  },
};
export default config;
```

The `persistedDocuments` option tells the `@graphql-codegen` tool to generate the trusted documents. The `hashAlgorithm` option specifies the hashing algorithm to use (you should always set it to `"sha256"` to override the default `sha1`, which is not as collision-resistant). The `addTypenameSelectionDocumentTransform` option adds the `__typename` field to the queries and mutations (which is required to make caching work with the URQL client).

You will typically already have a `predev` step to run the `@graphql-codegen` tool, so there is no need to change it.

  </TabItem>
</Tabs>

### Setting up the server

Setting up the server involves copying the trusted documents to the `trusted-documents` directory. The precise nature of this step depends on your particular setup. For example, consider that you use a separate repository for the server. Then, you will need a mechanism such as a CI/CD pipeline to copy the trusted documents to the server's repository (for example, by creating a pull request). If you have a monorepo setup, it is often as easy as augmenting the build process to copy the trusted documents to the server. For example, you could augment the `predev` script in the `package.json` file to copy the trusted documents to the server.

:::tip Check-in the trusted documents
It is a good idea to check in the trusted documents to the server's repository. This way, you can track changes to the trusted documents and have a record of the queries and mutations used by the client application.
:::

<Tabs groupId="client-choice">
  <TabItem value="apollo" label="Apollo Client" default>

```json
{
  ...
  "scripts": {
    "predev": "... && cp persisted-query-manifest.json ../api/trusted-documents && ...",
    ...
  },
  ...
}
```

  </TabItem>
  <TabItem value="urls" label="URQL Client" default>

```json
{
  ...
  "scripts": {
    "predev": "... && cp src/__generated__/persisted-documents.json ../api/trusted-documents && ...",
    ...
  },
  ...
}
```

  </TabItem>
</Tabs>

### Setting up the client

Finally, you need to set up the client to send only the hashes of the executable documents. The Apollo and URQL clients have built-in support that allows configuring the client to do so (and doesn't require changes to the rest of the client code).

<Tabs groupId="client-choice">
  <TabItem value="apollo" label="Apollo Client" default>

We will use the [persisted query link](https://www.apollographql.com/docs/react/api/link/persisted-queries/) to send only the hashes of the original executable document.

```ts
import { ApolloClient, createHttpLink, InMemoryCache } from "@apollo/client";
// highlight-start
import { generatePersistedQueryIdsFromManifest } from "@apollo/persisted-query-lists";
import { createPersistedQueryLink } from "@apollo/client/link/persisted-queries";
// highlight-end
const httpLink = createHttpLink({
  uri: process.env.NEXT_PUBLIC_GRAPHQL_URL,
});

// highlight-start
const persistedQueryLink = createPersistedQueryLink(
  generatePersistedQueryIdsFromManifest({
    loadManifest: () => import("/persisted-query-manifest.json"),
  })
);
// highlight-end

export const client = new ApolloClient({
  // highlight-next-line
  link: persistedQueryLink.concat(httpLink),
  cache: new InMemoryCache(),
  connectToDevTools: process.env.NEXT_PUBLIC_APOLLO_CONNECT_TO_DEV_TOOLS,
});
```

  </TabItem>
  <TabItem value="urls" label="URQL Client" default>

We will use the [`persistedFetchExchange`](https://commerce.nearform.com/open-source/urql/docs/advanced/persistence-and-uploads/) to send only the hashes of the executable document. The following is an example of how to set up the URQL Client to use the persisted fetch exchange.

```ts
import { Client, cacheExchange, fetchExchange } from "urql";
// highlight-next-line
import { persistedExchange } from "@urql/exchange-persisted";

export const client = new Client({
  url: process.env.NEXT_PUBLIC_GRAPHQL_URL,
  exchanges: [
    cacheExchange,
    // highlight-start
    persistedExchange({
      enforcePersistedQueries: true,
      enableForMutation: true,
      generateHash: (_, document) =>
        Promise.resolve(document["__meta__"]["hash"]),
    }),
    // highlight-end
    fetchExchange,
  ],
});
```

  </TabItem>
</Tabs>

Behind the scenes, the client will send only the hashes of the executable document to the server. The server will look up the executable document using the hash and execute it.

## Server enforcement of trusted documents

Let's consider how Exograph enforces trusted documents.

During the [build phase](../cli-reference/development/build.md), Exograph parses the content of the `trusted-documents` directory and includes a representation in the `exo_ir` file produced. You do not need anything special during production. Specifically, you do not need the `trusted-documents` directory in the production environment.

During execution, Exograph enforces the mode in the following way:

- In production mode, Exograph enforces trusted documents. _There is no way to opt out of this behavior_.
- In [development](../cli-reference/development/dev.md) or [yolo](../cli-reference/development/yolo.md), like in production mode, Exograph enforces trusted documents. This behavior allows testing the client with trusted documents without starting the server in production mode. However, you may opt out of this behavior by passing the `--enforce-trusted-documents=false` option. See the [development](../cli-reference/development/dev.md) and [yolo](../cli-reference/development/yolo.md) pages for more details.
- In development and yolo mode, Exograph makes an exception to queries its playground makes. The playground may send any executable document, even if it is not in the trusted documents. This behavior helps in exploring new queries and mutations. _This exception is not available in production mode._ Even if you enable introspection (and thus playground) in production, the server will still enforce trusted documents.
