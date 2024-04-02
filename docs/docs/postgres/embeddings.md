---
sidebar_position: 7
---

# Embeddings

Exograph supports embeddings using the [`pgvector`](https://github.com/pgvector/pgvector) extension. You can store embeddings in your Postgres database and query them using GraphQL queries. This feature forms the basis for many AI applications, such as recommendation systems, search engines, and anomaly detection.

## Prerequisites

To use embeddings in Exograph in local Postgres, you must install the `pgvector` extension. Please refer to the [pgvector documentation](https://github.com/pgvector/pgvector?tab=readme-ov-file#installation) for installation instructions.

If you use a managed Postgres service, please check with your provider if they have enabled the `pgvector` extension.

## Overview

Exograph introduces a new type `Vector`. Fields of this type provide the ability to:

- Creating and migrating the database schema.
- Supporting mutation APIs to store the embeddings in that field.
- Extending retrieval and ordering APIs to use the distance from the given vector.

To use embeddings in your application, declare a field of the `Vector` type in your schema.

```exo
@postgres
module DocumentModule {
    @access(true)
    type Document {
        @pk id: Int = autoIncrement()
        title: String
        content: String
        // highlight-next-line
        contentVector: Vector?
    }
}
```

The `Vector` type feature plays well with the rest of Exograph's capabilities. For example, you can apply access control to your entities, so searching and sorting automatically consider the user's access rights, thus eliminating a source of privacy issues. You can even specify [field-level access control](https://exograph.dev/docs/postgres/access-control#field-level-access-control) to, for example, expose the vector representation only to privileged users.

Using embeddings in your application involves:

1. Computing vector representation using an AI model like OpenAI's [`text-embedding-3-small`](https://platform.openai.com/docs/guides/embeddings/embedding-models) and storing/updating the vector fields using Exograph APIs
2. Querying for similar documents using Exograph's query APIs

Let's dive into these APIs.

## GraphQL APIs

Through the GraphQL API, the `Vector` type surfaces as a float array (`[Float!]`). This design choice simplifies interaction with the API from AI application and client code.

### Creating entities

To insert a document and its vector representation, you can use the following mutation:

```graphql
mutation($title: String!, $content: String!, contentVector: [Float!]!) {
    createDocument(data: {title: $title, content: $content, contentVector: $contentVector}) {
        id
    }
}
```

For instance, to insert a document with the title "Vacation policy", content "Company has unlimited PTOs and employees typically take four weeks each year", and its vector representation obtained from a provider, you would set the following variables:

```json
{
    "title": "Vacation policy",
    "content": "Company has unlimited PTOs and employees typically take four weeks each year",
    "contentVector": [...]
}
```

Inserting the vector representation along with the document is useful when you have the vector representation pre-computed. For example, if you have a largely unchanging document corpus, you can pre-compute the vector representation and insert it along with the document.

Typically, however, for other systems, you would use the mutation to insert the document without the vector representation and then update it with the vector representation later asynchronously. To enable this, you must mark the `Vector` field as optional (`Vector?`). Exograph will define the creation APIs to skip the vector field. You could set up an [interceptor](https://exograph.dev/docs/deno/interceptor) to track the documents that need embedding and update them when the vector representation is available.

### Updating entities

Exograph extends its update mutations to accept vector fields. To update the vector representation of a document, you can use the following mutation:

```graphql
mutation($id: Int!, contentVector: [Float!]!) {
    updateDocument(id: $id, data: {contentVector: $contentVector}) {
        id
    }
}
```

And pass it the document's `id` and the new vector representation. For instance, to update the vector representation of the document with `id` 1, you would set the following variables:

```json
{
    "id": 1,
    "contentVector": [...]
}
```

As you may have noticed, there isn't much difference between mutation APIs for a vector field and another scalar field.

### Querying entities

Now, we can query our documents. A common query with embedding is to retrieve the top matching documents. You can do it in Exograph with the following query:

```graphql
query topThreeSimilar($searchVector: [Float!]!) {
  documents(
    orderBy: { contentVector: { distanceTo: $searchVector, order: ASC } }
    limit: 3
  ) {
    id
    title
    content
  }
}
```

Limiting the number of documents is often sufficient for a typical search or RAG application. However, you can also use the `similar` operator to filter documents based on the distance from the search vector:

```graphql
query similar($searchVector: [Float!]!) {
  documents(
    where: {
      contentVector: {
        similar: { distanceTo: $searchVector, distance: { lt: 0.5 } }
      }
    }
  ) {
    id
    title
    content
  }
}
```

You can combine the `orderBy` and `where` clauses to return the top three similar documents only if they are within a certain distance:

```graphql
query topThreeSimilarDocumentsWithThreshold(
  $searchVector: [Float!]!
  $threshold: Float!
) {
  documents(
    where: {
      contentVector: {
        similar: { distanceTo: $searchVector, distance: { lt: $threshold } }
      }
    }
    orderBy: { contentVector: { distanceTo: $searchVector, order: ASC } }
  ) {
    id
    title
    content
  }
}
```

You can combine vector-based queries with other fields to filter and order based on other criteria. For example, you can filter based on the document's title and order based on the distance from the search vector:

```graphql
query topThreeSimilarDocumentsWithTitle(
  $searchVector: [Float!]!
  $title: String!
) {
  documents(
    where: { title: { eq: $title } }
    orderBy: { contentVector: { distanceTo: $searchVector, order: ASC } }
    limit: 3
  ) {
    id
    title
    content
  }
}
```

These filtering and ordering capabilities make it easy to focus on the business logic of your application and let Exograph handle the details of querying and sorting based on vector similarity.

## Customizing Embeddings

Until now, we have used the default settings for embeddings. However, Exograph provides several ways to customize embeddings:

- **Size**: The size of the vector. By default, Exograph uses a size of 1536, but you can specify a different size using the `@size` annotation. Exograph's schema creation and migration will factor in the vector size.

- **Indexing**: Creating indexes speeds up the search and ordering. Exograph's `@index` annotation on the `contentVector` field indicates the need to create an index. During schema creation (and migration), Exograph sets up a [Hierarchical Navigable Small World (HNSW)](https://en.wikipedia.org/wiki/Hierarchical_Navigable_Small_World_graphs) index.

- **Distance function**: The core motivation for using vectors is to find vectors similar to a target. There are multiple ways to compute similarity, and based on the field's characteristics, one may be more suitable than others. Since it is a field's characteristic, you can annotate `Vector` fields using the `@distanceFunction` annotation to specify the distance function. By default, Exograph uses the "cosine" distance function, but you can also use the "l2" distance function (L2 or Euclidean distance) or "ip" (inner product). Exograph will automatically use this function when setting up filters and ordering. It will also automatically factor in the distance function while setting up the index.

For example, to customize the `contentVector` field, you can use the following schema:

```exo
@postgres
module DocumentModule {
    @access(true)
    type Document {
        @pk id: Int = autoIncrement()
        title: String
        content: String

        // highlight-start
        @size(1536)
        @index
        @distanceFunction("l2")
        contentVector: Vector?
        // highlight-end
    }
}
```

With these annotations, Exograph will set the vector size to 1536, use the L2 distance function, and create an index on the `contentVector` field.
