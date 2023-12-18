---
sidebar_position: 1
---

# Overview

So far, we have defined types in a Postgres module. Now, it is time to reap its benefits.

In this section, we will explore all of these queries and mutations. Since queries and mutations return data, we will first take an [overview](overview.md) of how Exograph structures queries and mutations, and then we will look at [how to query](queries.md) and [how to mutate](mutations.md) data.

For each type, Exograph automatically infers queries and mutations. Specifically, Exograph infers three queries for each entity type:

- Get the entity by its primary key
- Get a list of entities,
- Get aggregate values such as the count or sum of a field.

Exograph also infers mutations:

- Create an entity
- Update an entity
- Delete an entity

It also creates a bulk version of the above mutations to work with multiple entities.

## Operation Return Types

When you execute a query, you get back data (duh!). The same thing applies to mutations, where, for example, you can get the ID of the entity you just created. Exograph defines a uniform way to return data from queries and mutations. This section will explain how the return type is structured.

Throughout this section, we will use the following persistence module for the concert management site.

```exo
@postgres
module ConcertModule {
  @access(true)
  type Concert {
    @pk id: Int = autoIncrement()
    title: String
    startTime: Instant
    endTime: Instant
    venue: Venue
  }

  @access(true)
  type Venue {
    @pk id: Int = autoIncrement()
    name: String
    concerts: Set<Concert>?
  }
}
```

Here, Exograph will infer the following queries (as well as mutations, which we will discuss later):

- `concert(id: Int!): Concert`
- `concerts(where: ConcertFilter, orderBy: ConcertOrdering, limit: Int, offset: Int): [Concert]`
- `concertAgg(where: ConcertFilter): ConcertAgg`
- `venue(id: Int!): Venue`
- `venues(where: VenueFilter, orderBy: VenueOrdering, limit: Int, offset: Int): [Venue]`
- `venueAgg(where: VenueFilter): VenueAgg`

The return type of each query mirrors the fields defined in the Exograph type. So in our example, we get the following types (shown in GraphQL schema definition syntax):

```graphql
type Concert {
  id: Int!
  title: String!
  content: String!
  startTime: Instant!
  endTime: Instant!
  venue: Venue!
}

type Venue {
  id: Int!
  name: String!
  concerts(
    where: ConcertFilter
    orderBy: ConcertOrdering
    limit: Int
    offset: Int
  ): [Concert]
  venueAgg(where: VenueFilter): VenueAgg
}
```

A few things to note:

- For each scalar field, you get an equivalent GraphQL field.
- For each relation field, you get a related entity or a list of entities depending on the cardinality of that field. You can apply filtering, order by, and pagination for any list type field.
- For each field of the list type, you also get another field for the aggregate value of that field. For example, if for the `concerts` field, you get the `concertsAgg` field of the `ConcertAgg` type, you can get the count of concerts, the sum of a field, etc.

##

With this structure, you can, for example, query a venue along with its concerts:

```graphql
venue(id: 10) {
  id
  name
  concerts {
    id
    title
    startTime
    endTime
    venue {
      id
    }
  }
}
```

This query will return a result such as the following:

```json
{
  "data": {
    "venue": {
      "id": 10,
      "name": "Symphony Hall",
      "concerts": [
        {
          "id": 10,
          "title": "First Concert",
          "startTime": "2023-01-01T15:00:00Z",
          "endTime": "2023-01-01T18:00:00Z",
          "venue": {
            "id": 10
          }
        },
        {
          "id": 11,
          "title": "Second Concert",
          "startTime": "2023-08-01T15:00:00Z",
          "endTime": "2023-08-01T18:00:00Z",
          "venue": {
            "id": 10
          }
        }
      ]
    }
  }
}
```

Exograph doesn't just restrict adding the `where` clause to the root query. For example, you can query for concerts starting after January 15, 2023 hosted at the venue with id 10:

```graphql
venue(id: 10) {
  id
  name
  concerts(where: {startTime: {gt: "2023-06-01T00:00:00Z"}}) {
    title
    startTime
    endTime
  }
}
```

Then you will get a result such as the following:

```json
{
  "data": {
    "venue": {
      "id": 10,
      "name": "Symphony Hall",
      "concerts": [
        {
          "title": "Second Concert",
          "startTime": "2023-08-01T15:00:00Z",
          "endTime": "2023-08-01T18:00:00Z"
        }
      ]
    }
  }
}
```

Often, you need to get a field of the list type multiple times, but each with a different filter. For example, you want to get all concerts hosted in the first half of 2023 and those hosted in the second half of 2023. You can do this by using aliases (without aliases, per GraphQL, Exograph will complain that the field `concerts` is defined multiple times):

```graphql
venue(id: 10) {
  id
  name
  firstHalfConcerts: concerts(
    where: {startTime: {gte: "2023-01-01T00:00:00Z", lt: "2023-07-01T00:00:00Z"}}
  ) {
    title
    startTime
    endTime
  }
  secondHalfConcerts: concerts(
    where: {startTime: {gte: "2023-07-01T00:00:00Z", lt: "2023-12-31T00:00:00Z"}}
  ) {
    title
    startTime
    endTime
  }
}
```

Equipped with this knowledge, you can now explore how to [query](queries.md) and [mutate](mutations.md) data.
