---
sidebar_position: 30
---

# Queries

Exograph automatically infers a set of queries for each type in a Postgres module. Specifically, it creates queries to:

- Obtain a single entity by its primary key
- Obtain a list of entities with optional filtering, ordering, and pagination
- Obtain aggregate information about the entities
- Obtain a single entity by any unique constraint

Exograph also infers a set of mutations, which we will defer to [the next section](mutations.md).

## Primary Key Query

When you want to view a single entity, for example, when displaying a particular concert, you can use the query to get the entity by its ID. Exograph creates a query named the "camelCased" version of the entity type name. For example, if the entity type is `Concert`, the query name will be `concert`, whereas if the entity type is `ShoppingCart`, the query name will be `shoppingCart`.

The query takes one argument: `id`, the entity's primary key, and returns a single optional entity. Then you can make queries such as this:

```graphql
concert(id: 5) {
  id
  title
}
```

## Collection Query

When you want to display a list of entities, for example, when displaying a list of concerts in a given year, you can use the query to get a list of entities. The query to get a list of entities is the "camelCased" version of the pluralized entity type. For example, if the entity type is `Concert`, the query name will be `concerts`, whereas if the entity type is `ShoppingCart`, the query name will be `shoppingCarts`.

Recall from the discussion of the [`@plural`](../customizing-types.md#pluralization) that if you use this annotation, Exograph will use that as the plural version; otherwise, the system will use its algorithm to compute one. So if you had provided `@plural("people") type Person`, the query to get multiple entities will be `people`.

Each query takes four arguments, all of which are optional:

- `where`: an expression to filter the list of entities
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
- `ilike`: Similar to `like`, but matches the pattern, ignoring the case.
- `startWith`: The string field starts with the given pattern (it is a shortcut to using `like` along with a pattern that ends with a `%`).
- `endWith`: The string field ends with the given pattern (a shortcut to using `like` along with a pattern that starts with a `%`).

:::note The `Vector` type
The `Vector` scalar type gets special treatment in Exograph. You can use the `similar` operator to filter documents based on the distance from the search vector. We will explore this in more detail in the [Embeddings](../embeddings) section.
:::

In Postgres, the JSON fields are useful for storing arbitrary data without precise control over its schema. In a way, such fields enable treating the database as a document store. Exograph offers a few operators to match against the content of such fields. Let's assume that you want to keep some metadata about your concerts, and you have a `metadata` JSON field with the current value in the database as `{"a": 1, "b}`. You can use the following operators:

| Operator       | Description                                                                            | Matching Examples                                                                       | Non-matching Examples                                                                 |
| -------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `contains`     | The JSON field contains the given value                                                | `{ metadata: { contains: { "a": 1 } } }`                                                | `{ metadata: { contains: { "a": 2 } } }`                                              |
| `containedBy`  | The JSON field is contained by the given value (this is `contains` with sides flipped) | `{ metadata: { containedBy: { "a": 1, "b": 2, "c": 3 } } }`                             | `{ metadata: { containedBy: { "a": 1, "b": 1 } } }`                                   |
| `matchKey`     | The JSON field contains the given key                                                  | `{ metadata: { matchKey: "a" }}`                                                        | `{ matchKey: "c" }`                                                                   |
| `matchAllKeys` | The JSON field contains all the given keys                                             | `{ { metadata: { matchAllKeys: ["b"] } }`, `{ { metadata: matchAllKeys: ["a", "b"] } }` | `{ { metadata: matchAllKeys: ["c"] } }`, `{ { metadata: matchAllKeys: ["a", "c"] } }` |
| `matchAnyKey`  | The JSON field contains any of the given keys                                          | `{ { metadata: { matchAnyKey: ["a", "c"] } }`                                           | `{ { metadata: matchAnyKey: ["c"] } }`                                                |

### `orderBy`

The `orderBy` expression is a list of fields to order the list of entities. It will apply the ordering in the provided sequence. For example, the following expression will return all concerts ordered by the `date` field in descending order and then by the `title` field in ascending order:

```graphql
orderBy: [{ date: DESC }, { title: ASC }]
```

:::note The `Vector` type
The `Vector` scalar type gets special treatment in Exograph. You can sort documents based on the distance from the search vector. We will explore this in more detail in the [Embeddings](../embeddings) section.
:::

### `limit` and `offset`

The `limit` and `offset` parameters enable paginating the list of entities. Each of them takes an integer value. For example, the following expression will return the first ten concerts after skipping the first 5:

```graphql
concerts(limit: 10, offset: 5) {
  ...
}
```

## Unique Constraint Query

If a type consists of `@unique` fields, Exograph infers one query per unique constraint. Each such query takes all the fields of the unique constraint as arguments and returns a single optional entity (the same way as the primary key query). Each query follows the naming convention of

```graphql
<lowerCamelCasedTypeName>By<upperCamelCasedUniqueConstraintName>(<uniqueConstraintFields>): <TypeName>
```

For example, consider the following type:

```exo
type Concert {
  ...
  @unique name: String
}
```

Exograph will infer the `concertByName` query that takes the `name` field as an argument and returns a single optional concert. You can use this query as follows:

```graphql
concertByName(name: "The Beatles") {
  ...
}
```

If you have marked a combination of fields as unique, Exograph will infer a query that takes all those fields as arguments. For example, consider the following type:

```exo
type Person {
  ...
  @unique("email") emailId: String
  @unique("email") emailDomain: String
}
```

Exograph will infer the `personByEmail` query that takes the `emailId` and `emailDomain` fields as arguments and returns a single optional entity.

You can use this query as follows:

```graphql
personByEmail(emailId: "john", emailDomain: "example.com") {
  ...
}
```

Similarly, if you have a field with multiple unique constraints, Exograph will infer a query for each unique constraint. For example, consider the following type:

```exo
type Person {
    @unique("primary_email") primaryEmailId: String
    @unique("secondary_email") secondaryEmailId: String?
    @unique("primary_email", "secondary_email") emailDomain: String
}
```

Here, we have two unique constraints. Therefore, Exograph will infer two queries: `personByPrimaryEmail` and `personBySecondaryEmail`, each taking the fields of the corresponding unique constraint as arguments. You can use these queries as follows:

```graphql
personByPrimaryEmail(primaryEmailId: "alex", emailDomain: "example.com") {
  ...
}

personBySecondaryEmail(secondaryEmailId: "alex", emailDomain: "example.com") {
  ...
}
```

If you mark a relation field's "one" side as unique, Exograph will infer a query that takes the primary key as the argument. For example, consider the following type:

```exo
type Rsvp {
  ...
  @unique("registration") concert: Concert // "one" side of the relation
  @unique("registration") email: String
}
```

Exograph will infer a query named `rsvpByRegistration` that takes the `concert` and `email` fields as arguments and returns a single optional entity. You can use this query as follows:

```graphql
rsvpByRegistration(concert: {id: 5}, email: "john@example.com") {
  ...
}
```

## Aggregate Query

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

Exograph provides the `count` aggregate for any field type, `sum`, `avg`, `max`, `min` for numeric field types, `min`, `max` for string fields, and `avg` for [vector](../embeddings) fields. For example, if you wanted to know the total number of tickets sold for all concerts, you could use the following query:

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
