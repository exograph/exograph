---
sidebar_position: 2
---

# Queries

Let's explore queries created for Postgres types. We will explore three kinds of queries: single element, collection, and aggregate. We will defer mutations to [the next section](mutations.md).

## Get an entity by the primary key

When you want to view a single entity, for example, when displaying a particular concert, you can use the query to get the entity by its ID. Exograph creates a query named as the "camelCased" version of the entity type name. For example, if the entity type is `Concert`, the query name will be `concert`, whereas if the entity type is `ShoppingCart`, the query name will be `shoppingCart`.

The query takes one argument: `id`, which is the entity's primary key and returns a single entity. Then you can make queries such as this:

```graphql
concert(id: 5) {
  id
  title
}
```

## Get a list of entities

When you want to display a list of entities, for example, when displaying a list of concerts in a given year, you can use the query to get a list of entities. The query to get a list of entities is the "camelCased" version of the pluralized entity type. For example, if the entity type is `Concert`, the query name will be `concerts`, whereas if the entity type is `ShoppingCart`, the query name will be `shoppingCarts`.

Recall from the discussion of the [`@plural`](../customizing-types.md#pluralization) that if you use this annotation, Exograph will use that as the plural version; otherwise, the system will use its algorithm to compute one. So if you had provided `@plural("people") type Person`, the query to get multiple entities will be `people`.

Each query takes four arguments, all of which are optional:

- `where`: a filter expression that is used to filter the list of entities
- `orderBy`: a list of fields to order the list of entities
- `limit`: the maximum number of entities to return
- `offset`: the number of entities to skip

Let's take a look at each of these.

### `where`

The `where` expression is a boolean expression evaluated in the entity's context. For example, if the entity type is `Concert`, the `where` expression can access the entity's fields, such as `id`, `title`, etc.

For example, if you want to get all concerts of the "rock" genre, you would use the following query:

```graphql
concerts(where: {genre: {eq: "rock"}}) {
  ...
}
```

Whereas, if you wanted to get all concerts whose title start with "The", you would use the following query (we will examine all operators such as `startsWith` later in this section):

```graphql
concerts(where: {title: {startsWith: "The"}}) {
  ...
}
```

What if you want both criteria? You can provide each of those filters in a comma-separated list. This has the effect of combining individual filters with a logical `and`:

```graphql
concerts(where: {genre: {eq: "rock"}, title: {startWith: "The" }}) {
  ...
}
```

You could alternatively provide the `and` operator explicitly to the same effect:

```graphql
concerts(where: {and: [{genre: {eq: "rock"}, title: {startWith: "The" }}]}) {
  ...
}
```

Likewise, as you probably already guessed, you can use the `or` operator to perform the logical or. For example, to get the concerts with the "rock" genre **or** start their title in "The", you could use the following query:

```graphql
concerts(where: {or: [{genre: {eq: "rock"}, title: {startWith: "The" }}]}) {
  ...
}
```

Exograph also provides a `not` operator to negate the condition. For example, if you wanted to get all concerts that do _not_ start with "The", you can use the following query:

```graphql
concerts(where: {not: {title {startsWith: "The"}}}) {
  ...
}
```

For a field of any kind, you can use the following operators:

- `eq`: equal to
- `neq`: not equal to

For numeric fields as well as date fields, you can also use the following operators:

- `gt`: greater than
- `gte`: greater than or equal to
- `lt`: less than
- `lte`: less than or equal to

For string fields, you can also use the following operators:

- `like`: To compare a string field to a pattern. The pattern can contain the `%` character, which matches any sequence of characters. For example, the pattern `%concert%` matches any string that contains the word "concert".
- `ilike`: Similar to `like`, but match the pattern ignoring the case.
- `startWith`: The string field starts with the given pattern (it is a shortcut to using `like` along with a pattern that ends with a `%`).
- `endWith`: The string field ends with the given pattern (a shortcut to using `like` along with a pattern that starts with a `%`).

In Postgres, the JSON fields are useful to store arbitrary data without precise control over its schema. In a way, such fields allow treating the database as a document store. Exograph offers a few operators to match against the content of such fields. Let's assume that you want to keep some metadata about your concerts, and you have a `metadata` JSON field with the current value in the database as `{"a": 1, "b}`. You can use the following operators:

| Operator       | Description                                                                            | Matching Examples                                                                       | Non-matching Examples                                                                 |
| -------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `contains`     | The JSON field contains the given value                                                | `{ metadata: { contains: { "a": 1 } } }`                                                | `{ metadata: { contains: { "a": 2 } } }`                                              |
| `containedBy`  | The JSON field is contained by the given value (this is `contains` with sides flipped) | `{ metadata: { containedBy: { "a": 1, "b": 2, "c": 3 } } }`                             | `{ metadata: { containedBy: { "a": 1, "b": 1 } } }`                                   |
| `matchKey`     | The JSON field contains the given key                                                  | `{ metadata: { matchKey: "a" }}`                                                        | `{ matchKey: "c" }`                                                                   |
| `matchAllKeys` | The JSON field contains all the given keys                                             | `{ { metadata: { matchAllKeys: ["b"] } }`, `{ { metadata: matchAllKeys: ["a", "b"] } }` | `{ { metadata: matchAllKeys: ["c"] } }`, `{ { metadata: matchAllKeys: ["a", "c"] } }` |
| `matchAnyKey`  | The JSON field contains any of the given keys                                          | `{ { metadata: { matchAnyKey: ["a", "c"] } }`                                           | `{ { metadata: matchAnyKey: ["c"] } }`                                                |

### `orderBy`

The `orderBy` expression is a list of fields to order the list of entities. It will apply the ordering in the provided sequence. For example, the following expression will return all concerts ordered by the `date` field in descending order, and then by the `title` field in ascending order:

```graphql
orderBy: [{ date: DESC }, { title: ASC }]
```

### `limit` and `offset`

The `limit` and `offset` parameters are used to paginate the list of entities. Each of them takes an integer value. For example, the following expression will return the first ten concerts after skipping the first 5:

```graphql
concerts(limit: 10, offset: 5) {
  ...
}
```

## Aggregate Queries

In addition to the queries to get a list of entities, Exograph also provides queries to obtain aggregate information about the entities. For example, if you want to get the total number of concerts, you can use the following query:

```graphql
concertsAgg {
  id {
    count
  }
}
```

The result of this query will be:

```json
{
  "concertsAgg": {
    "id": {
      "count": 100
    }
  }
}
```

If you wanted to know how many concerts were hosted in 2020, you could use the following query:

```graphql
concertsAgg(where: { date: { gte: "2020-01-01", lt: "2021-01-01" } }) {
  id {
    count
  }
}
```

Exograph provides the `count` aggregate for any field type, `sum`, `avg`, `max`, `min` for numeric field types, and `min`, `max` for string fields. For example, if you wanted to know the total number of tickets sold for all concerts, you could use the following query:

```graphql
concertsAgg {
  ticketsSold {
    sum
  }
}
```

As noted earlier, you can nest an aggregate field inside another query to get aggregate information about the nested entities. For example, if you wanted to display a table with the concert name and total RSVPs for each, you could use the following query:

```graphql
concerts {
  title
  rsvpsAgg {
    id {
      count
    }
  }
}
```

You will get a result like this:

```json
{
  "concerts": [
    {
      "title": "The Beatles",
      "rsvpsAgg": {
        "id": {
          "count": 1000
        }
      }
    },
    {
      "title": "The Rolling Stones",
      "rsvpsAgg": {
        "id": {
          "count": 2000
        }
      }
    }
  ]
}
```
