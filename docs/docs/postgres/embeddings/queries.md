---
sidebar_position: 30
---

# Querying

Exograph's support for embeddings extends queries to include vector-based filtering and ordering, so once you have familiarized yourself with Exograph's querying capabilities, you can start using embeddings easily. It also provides a way to retrieve the distance of a document from a target vector and aggregate information about the documents.

As we've seen in the [mutations](mutations.md) section, in the GraphQL API, the `Vector` type surfaces as a float array (`[Float!]`).

## Filtering and ordering

Exograph extends its querying capabilities to support filtering and ordering based on vector embeddings. Both capabilities allow specifying a target vector and filtering or ordering based on the distance from the target vector.

### Retrieving closest documents

A common query with embedding is to retrieve the top matching documents. Exograph supports this with the `orderBy` clause (along with the existing support for `limit` and `offset`). For example, to retrieve the top three documents similar to a search vector, you can use the following query:

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

Here, the `orderBy` clause for the vector field accepts a `distanceTo` operator to specify the target vector and `order` to specify the sorting order. Exograph will automatically use the distance function specified for the field using the `@distanceFunction` annotation (see [Customizing Embeddings](customizing)).

### Filtering based on distance

Limiting the number of documents is often sufficient for a typical search or RAG application. However, sometimes, you want to ensure you retrieve only documents within a certain distance. For this, you can also use the `similar` operator to filter documents based on the distance from the search vector:

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

The `similar` operator accepts a `distanceTo` operator to specify the target vector and a `distance` operator to specify the distance condition. The `distance` operator allows you to specify the comparison operator (`lt`, `lte`, `gt`, `gte`, `eq`) and the distance value. Like the `orderBy` clause, Exograph will automatically use the distance function specified for the field.

### Combining filters and ordering

You can combine the `orderBy` and `where` clauses to return the closest documents within a certain distance. For example, you can retrieve the top three similar documents only if they are within a certain distance:

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

### Combining other fields with vector-based queries

You can combine vector-based queries with other fields to filter and order based on other structured fields. This is often useful to narrow the search space based on structured data.

For example, you can filter based on the document's title along with a similarity filter and order based on the distance from the search vector:

```graphql
query topThreeSimilarDocumentsWithTitle(
  $searchVector: [Float!]!
  $title: String!
  $threshold: Float!
) {
  documents(
    where: {
      title: { eq: $title }
      contentVector: {
        similar: { distanceTo: $searchVector, distance: { lt: $threshold } }
      }
    }
    orderBy: { contentVector: { distanceTo: $searchVector, order: ASC } }
    limit: 3
  ) {
    id
    title
    content
  }
}
```

Here, we filter documents based on title equality and similarity to the search vector and order them based on the distance from the search vector.

## Finding the distance

When querying for similar documents, you may want to know the distance of each document from the search vector. Exograph supports this by returning the distance with the document through a special `<field-name>Distance` name field. This field accepts one argument `to` of the vector type to specify the target vector and returns a float value representing the distance.

```graphql
{
  documents {
    id
    title
    content
    # highlight-next-line
    contentVectorDistance(to: $searchVector)
  }
}
```

Here, the `contentVectorDistance` field returns the distance of each document's `contentVector` field from the search vector. You can use this field to post-process or to display an indication of relevance to the user.

## Finding aggregate information

In addition to retrieving individual documents, you may want to find aggregate information about the documents. For example, you may want to compute the average vector. This is useful for classification problems where you want to find a representative vector for a set of documents. Then, when a new vector comes into the system, you can compare it with the average vector to classify it.

Exograph supports this through the [aggregation API](../operations/queries.md#aggregate-query). Currently, it supports only the `avg` aggregation function for vector fields (besides `count`, which all fields support).

```graphql
{
  documents(where: ...) {
    contentVectorAgg {
      avg
    }
  }
}
```

Here, the `contentVectorAgg` field returns the average vector of all documents that match the `where` condition.
