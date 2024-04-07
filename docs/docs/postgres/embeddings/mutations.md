---
sidebar_position: 20
---

# Storing and updating

Exograph extends its mutation APIs to support storing and updating vector embeddings. Through the GraphQL API, the `Vector` type surfaces as a float array (`[Float!]`). This design choice simplifies interaction with the API from AI application and client code.

### Creating entities

To insert a document and its vector representation, you can use the following mutation:

```graphql
mutation ($title: String!, $content: String!, $contentVector: [Float!]!) {
  createDocument(
    data: { title: $title, content: $content, contentVector: $contentVector }
  ) {
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
mutation ($id: Int!, $contentVector: [Float!]!) {
  updateDocument(id: $id, data: { contentVector: $contentVector }) {
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
