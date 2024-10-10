---
sidebar_position: 2
---

# Defining Types

Types are the building blocks of a Postgres module since they define the entities you want to persist in your database. A type consists of fields. A field may be an Exograph-defined scalar type such as `Int`, `Float`, or `Uuid`. It may also be another type defined in the Postgres module, in which case it represents a relationship between the types.

Exograph maps each type to a table in the database and each field to a column of a table or a relation to another table. Using the convention over configuration approach, Exograph will automatically deduce the appropriate table names, column names, column types, foreign key constraints, etc. Usually, the automatically deduced mapping should suffice, especially for a green field project. However, if necessary, you may [customize](customizing-types.md) them.

Exograph processes types in the exo file to create appropriate [queries](operations/queries.md) and [mutations](operations/mutations.md) and apply [access control rules](access-control.md). The Postgres plugin doesn't allow you to define your own queries (however, you may use the [Deno module](/deno/overview.md) to do so).

## Defining a type

A type is defined using the `type` keyword. You have seen the `Concert` type a few times by now. Let's look at it in detail.

```exo
type Concert {
  @pk id: Int = autoIncrement()
  description: String?
  title: String
  price: Float
  published: Boolean = false
}
```

The `Concert` type defines several fields.

- Since the `id` field carries the `@pk` annotation, it is designated as the primary key of the `Concert` type. Due to the default value set to the `autoIncrement` function, Exograph, in collaboration with the database, will assign it the next value from a sequence. Behind the scenes, Exograph will map this field to a `SERIAL` column in the database. We will see how to customize such a primary key to, for example, map it to a `BIGSERIAL` or use a `Uuid` in the [next section](customizing-types.md). Designating a field as the primary key is a requirement for Exograph to generate the appropriate queries and mutations.

- The `description` field is of type `String`. The field is marked as nullable since the type carries the `?` suffix. This influences the Exograph-generated mutations such that the `description` field will be optional when creating a new concert.

- The `title` field is of type `String`. It is not nullable since it doesn't carry the `?` suffix. This influences mutations such that the `title` field will be required when creating a new concert. We will examine this in detail in the [mutations](operations/mutations.md) section.

- Likewise, the `price` field is of type `Float` and is not nullable. The `Float` type seems not quite appropriate for representing a price. A more appropriate type would be `Decimal`, as we will see later.

- The `published` field is of type `Boolean`. The field's default value is set to `false`. The mutations that create a concert will mark the `published` field as optional and use the default value if you don't specify it.

:::note
By default, Exograph will assume each field is required (i.e., not nullable). This is a safer assumption since it will prevent you from creating a record with a null value in a non-nullable field. It also makes it a backward-compatible change if you make a field nullable later.

Exograph, in this regard, follows TypeScript (which is also why we use the `?` suffix to mark a field as nullable) as well as most other languages (in Rust or Scala, for example, you have to declare such types using an `Option<>` generic type). However, this is a departure from the GraphQL spec, which assumes all fields are nullable (and you need to mark a field with an `!` suffix to mark it as required).
:::

## Defining a scalar field

In the example above, we have seen how to define fields of scalar types: `Int`, `String`, `Float`, and `Boolean`. Exograph supports the following scalar types (and we will add more in the future):

| Type              | Description                                                                                    | Example                                  |
| ----------------- | ---------------------------------------------------------------------------------------------- | ---------------------------------------- |
| `Int`             | An integer type (the size depends on several customizable factors).                            | `1`, `2`, `3`                            |
| `Float`           | A floating point type (the size depends on several customizable factors).                      | `1.0`, `2.0`, `3.0`                      |
| `Decimal`\*       | A decimal type with precise value (the precision can be specified). Useful for money and such. | `"10.99"`, `"1.9999"`                    |
| `String`          | A string type.                                                                                 | `"नमस्ते"`, `"world"`                    |
| `Boolean`         | A boolean type.                                                                                | `true`, `false`                          |
| `Uuid`\*          | A universally unique identifier type.                                                          | `"f81d4fae-7dec-11d0-a765-00a0c91e6bf6"` |
| `LocalDate`\*     | A date type.                                                                                   | `"2021-01-01"`                           |
| `LocalDateTime`\* | A date and time type.                                                                          | `"2021-07-06T20:08:47"`                  |
| `LocalTime`\*     | A time type.                                                                                   | `"14:30:15"`                             |
| `Instant`\*       | A date and time type along with timezone                                                       | `"2021-07-06T20:08:47.1234567-07:00"`    |
| `Json`\*          | A JSON type.                                                                                   | `{"hello": "world"}`                     |
| `Blob`\*          | An encoded binary data                                                                         | `"iVBORw0KGgoAAAANSUhEUgAAABgAAAAWC..."` |
| `Vector`\#        | A vector type.                                                                                 | `[1.0, 2.0, 3.0]`                        |

`*` Accepted and returned as a string through the GraphQL API but stored as the corresponding type in the database.  
`#` Accepted and returned as a float array through the GraphQL API but stored as the corresponding type in the database.

:::note The `Vector` type
The `Vector` type is somewhat different than the other scalar types in the way it supports filtering and ordering, which we will explore in the [embeddings](embeddings/overview.md) section.
:::

Besides the plain scalar types, Exograph also supports Arrays of scalar types. For example, you can define a field of type `Array<String>` to store a list of strings.

## Defining a relationship

A type rarely stands alone; it becomes interesting when it relates to others. These relationships are the reason why we use a relational database. This is also where GraphQL shines by allowing us to query an entity along with its related data. This section will look at how to define a relationship between two types.

### One-to-many and many-to-one relationship

The most common form of relationship is the one-to-many and many-to-one relationships. For example, a concert is held in a venue. If we look at it from a venue's perspective, it has a list of concerts. We can define such a relationship by merely including fields of the right kind.

```exo
type Concert {
  @pk id: Int = autoIncrement()
  description: String?
  title: String
  price: Float
  published: Boolean = false
  // highlight-next-line
  venue: Venue
}

type Venue {
  @pk id: Int = autoIncrement()
  name: String
  // highlight-next-line
  concerts: Set<Concert>?
}
```

We only needed to include a field of type `Venue` in the `Concert` type and a field of type `Set<Concert>` in the `Venue` type. In other words, we established a one-to-many relationship from `Venue` to `Concert` and a many-to-one relationship from `Concert` to `Venue`.

:::note
Why `Set` and not an array?

An array is an ordered sequence of values without restrictions on how many times a value can appear. On the other hand, a set is an unordered collection of values without duplicates. This is precisely what we need to express a relationship between two types. For example, when you query a venue, you want to get all the concerts held there. You don't care about the order of the concerts (if you do, you can always provide an `orderBy` query parameter as [we will see later](operations/queries.md)), and you don't want to see the same concert twice.

Exograph does support `Array` if you need the array semantics, as we have seen in the [scalar fields](#defining-a-scalar-field) section, but its usage is limited to scalar types only.
:::

Note also that the `concerts` field is optional since a venue may not have any concerts. This way, we are not forced to specify a value for the `concerts` field when creating a venue. We will explore more about this in the [mutations](operations/mutations.md) section.

### Many-to-many relationship

A many-to-many relationship involves two types, each related to multiple instances of the other. For example, a concert may feature multiple artists, and an artist may perform in multiple concerts. In real-world scenarios, some data is almost always associated with the relationship. For example, the artist may be a concert's main or supporting artist. This calls for an intermediate type to hold the data associated with the relationship. Exograph supports many-to-many relationships indirectly by allowing you to define a relationship between two types through an intermediate type.

```exo
type Concert {
  @pk id: Int = autoIncrement()
  ...
  // highlight-next-line
  performances: Set<Performance>?
}

type Artist {
  @pk id: Int = autoIncrement()
  ...
  // highlight-next-line
  performances: Set<Performance>?
}

type Performance {
  @pk id: Int = autoIncrement()
  // highlight-next-line
  artist: Artist
  // highlight-next-line
  concert: Concert
  isMainArtist: Boolean
}
```

Effectively, we have defined two one-to-many relationships between `Concert` and `Performance` and between `Artist` and `Performance`.

You may use the `@unique` annotation to mark the combination of the two fields that make a relationship unique. For example, in the above example, we may want to ensure that the performance uniquely identifies an artist and a concert. We can do so by marking the `artist` and `concert` fields with the `@unique` annotation.

```exo
type Performance {
  @pk id: Int = autoIncrement()
  // highlight-next-line
  @unique("relation") artist: Artist
  // highlight-next-line
  @unique("relation") concert: Concert
  isMainArtist: Boolean
}
```

Please see the [uniqueness](customizing-types.md#constraining-uniqueness) section for more details on using the `@unique` annotation.

### One-to-one relationship

One-to-one relationships are uncommon in practice but have their usage. In a typical situation, you would define a one-to-one relationship with one of the sides marked as optional. This avoids the chicken-or-the-egg situation: which instance shall you create first? With one side marked optional, you can create an instance with the optional side and then create an instance with the required side. For example, a user may have an optional membership, but a membership must have an associated user. Let's see how we can define such a relationship.

```exo
type User {
  @pk id: Int = autoIncrement()
  ...
  // highlight-next-line
  membership: Membership?
}

type Membership {
  @pk id: Int = autoIncrement()
  // highlight-next-line
  user: User
  ...
}
```

Here, we have defined a one-to-one relationship between `User` and `Membership`. The `membership` field in `User` is optional, and the `user` field in `Membership` is required. This arrangement allows us to create a user first (without a membership) and then create a membership for the user by providing it with the user as the `user` field. So, while we can't solve the chicken-or-the-egg problem, we can solve the user-or-the-membership problem: the user always comes first!

So far, we have explored how to create types with scalar fields and define relationships between types using Exograph's default mapping to the database. [In the next section](configuration.md), we will zoom into customizing the mapping.
