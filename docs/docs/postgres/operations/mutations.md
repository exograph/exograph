---
sidebar_position: 40
---

# Mutations

Mutations allow creating, updating, and deleting data in your database. For each entity type, Exograph creates two flavors of each mutation: one to work with a single entity and another with multiple entities. All mutations return the data they mutated, and the type returned is identical to those by queries. For example, the `createConcert`, `updateConcerts`, and `deleteConcerts` mutations return the `Concert` type, whereas their bulk counterparts return `[Concert]`. Please refer to the [overview](overview.md) section for more details.

## Creating data

To create a new entity, Exograph offers two mutations: `create<EntityType>` and `create<PluralizedEntityName>`. The first allows creating a single entity, whereas the other allows creating multiple. For example, Exograph will offer the `createConcert` and `createConcerts` mutations for the `Concert` entity type.

:::tip
As discussed in the [customizing types](../customizing-types.md) section, if you supply the `@plural` annotation, Exograph will use that as the pluralized mutation name. So, for example, if you annotate the `Person` type with `@plural("people")`, the pluralized mutation name will be `createPeople`. The same scheme applies to [update](#updating-data) and [delete](#deleting-data) mutations as well.
:::

Both create mutations use the same input type of the `<EntityName>CreateInput` form. For example, for the `createConcert` mutation, the input type is `ConcertCreationInput`. It has all the fields of the entity type. However, if the primary key is of the `Int` type set to `autoIncrement()`, it will not be in the input type. Every field marked optional in your type definition is also optional in the input type. The singular form mutation takes this type, whereas the multiple entity version takes an array.

:::tip Special treatment for `Uuid` primary keys
If your entity type has a primary key of the `Uuid` type, it may still be supplied in the input data. This allows client-generated UUIDs. If the client does not provide the primary key, Exograph will generate one (due to the default value of `generate_uuid()` or `uuidGenerateV4()` or `uuidGenerateV7()`).
:::

### Creating a single entity

Given this mutation, you can create a concert as follows:

```graphql
mutation {
  createConcert(data: {
    title: "My concert",
    startTime: "2023-01-01T15:00:00Z",
    venue: {id: 1}
  }) {
    id
    ...
  }
}
```

Since the mutation returns the created entity, you can retrieve any fields. Typically, the client will retrieve at least the primary key of the created entity since this will be the only chance for later use.

### Creating multiple entities

If you want to create multiple concerts, you can do so as follows:

```graphql
mutation {
  createConcerts(
    data: [
      {
        title: "My concert"
        startTime: "2023-10-01T15:00:00Z"
        venue: { id: 1 }
      }
      {
        title: "My concert 2"
        startTime: "2023-11-01T15:00:00Z"
        venue: { id: 3 }
      }
    ]
  ) {
    id
  }
}
```

Here, the return value is an array of the created entities matching the order of the input array.

:::note Multiple Mutations
Let's briefly detour to understand how Exograph works with multiple mutations submitted in a GraphQL payload. Consider the following payload, where the intention is to create a concert and an artist:

```graphql
createConcert(data: {...}) {
  ...
}

createArtist(data: {...}) {
  ...
}
```

Exograph will execute mutations in the order they appear in the payload. Here, Exograph will first create the concert and then create the artist. Exograph also sets up a transaction boundary around all the mutations in the payload. If one of the operations fails, any operation executed before that will also be rolled back (Exograph will not execute subsequent operations). This transaction arrangement ensures that each submitted operation is atomic.
:::

Sometimes, you must create associated entities in the same request. For that, you need to use nested creation.

### Nested creation

Consider creating a concert. You will typically also want to create performances along with it. Without nested creation, you would have to create the concert first, then create the performances, passing it the id of the concert you created. Not only would you have to make two requests to the server, but also will execute each operation in a separate transaction. If the second transaction fails, you will have a concert without artists.

With nested creation, you can create the concert and performances in one request and one transaction, which means the concert will be created along with artists atomically (either both succeed or the system is left in the original state). To do so, you can use the `createConcert` mutation as follows:

```graphql
mutation {
  createConcert(data: {
    title: "My concert",
    startTime: "2023-01-01T15:00:00Z",
    venue: {id: 1},
    performances: [
        {artist: {id: 1}, rank: 1, role: "main"},
        {artist: {id: 2}, rank: 2, role: "main"},
        {artist: {id: 3}, rank: 3, role: "accompanying"}
      ]
  }) {
    id
    ...
  }
}
```

Here the new concert will have three artists in it.

If you wonder if this could have been achieved by passing multiple mutations in the same payload, the answer is no. The reason is that the `createConcert` would return the newly created concert, and you will need the id returned to create the performances. But GraphQL cannot express passing data returned from one mutation to another in the same request. The only way to do so is to use nested creation.

## Updating data

To update an existing entity, Exograph offers two mutations: `update<EntityType>` and `update<PluralizedEntityName>`. The first allows updating one, whereas the second allows multiple entities. For example, the `updateConcert` mutation takes the `ConcertUpdateInput` type, which has all the fields of the `Concert` type except the `id` field. The `updateConcerts` mutation takes an array of `ConcertUpdateInput` objects.

The update mutations take the data argument of the `<EntityType>UpdateInput` type. This type is similar to the `<Entity>CreationInput`, except for two differences:

- it does not have the `id` field. Since you are updating an existing entity, you don't need to supply the primary key.
- all other fields are optional, so you can pass only the fields you want to update.

### Updating a single entity

Given this mutation, you can update a concert as follows:

```graphql
mutation {
  updateConcert(id: 1, data: {
    name: "My concert",
    startTime: "2023-01-01T15:00:00Z",
    venue: {id: 1}
  }) {
    ...
  }
}
```

Like the [query to get a single entity](queries.md#primary-key-query), if the entity type has a composite primary key, you must supply all the fields of the primary key as arguments to the mutation. For example, if the `Person` type has a composite primary key of `firstName` and `lastName`, you must supply both `firstName` and `lastName` as arguments to the mutation.

```graphql
mutation {
  updatePerson(firstName: "John", lastName: "Doe", data: {age: 30}) {
    ...
  }
}
```

### Updating multiple entities

If you want to update multiple concerts, you can do so as follows. In the following, the goal is to move all concerts hosted in venue 3 to venue 2.

```graphql
mutation {
  updateConcerts(where: {venue: {id: {eq: 3}}}, data: { venue: {id: 2} }) {
    ...
  }
}
```

We supplied the `where` argument to filter the concerts to be updated, which is the same as the one used to filter data in the queries in the [earlier section](queries.md#collection-query). The `data` argument supplies the new values for the fields. Here, since all we want is to change the venue, we only provide the `venue` field (thus leaving the other fields as they are).

### Nested updates

When you update an entity, you may also have to create new associated entities or update or delete existing ones. Exograph's nested update support lets you do all this in one go.

Consider a concert editor. You will invoke the `updateConcert` mutation to save a concert. Here, during editing, the user may have added a new artist to the concert, updated the role of an existing artist, or removed an artist from the concert. With nested updates, you can do all of this in one go. Without the nested update support, you will have the same issues discussed in the [nested creation](#nested-creation) section.

```graphql
updateConcert(id: 1, data: {
  name: "My concert",
  startTime: "2023-02-01T16:00:00Z",
  venue: {id: 1},
  performances: {
    create: [
      {artist: {id: 1}, role: "Singer"},
      {artist: {id: 2}, role: "Guitarist"}
    ],
    update: [
      {id: 3, role: "Drummer"}
    ],
    delete: {id: 4}
  }
}) {
  ...
}
```

Here, we add two new artists: id 1 as a singer and id 2 as a guitarist. We also update the role of an existing artist with id 3 to "drummer". Finally, we delete an artist with id 4. All of this is done in one go.

There is one more detail to note here. The `performances` added will automatically have its concert id set to one updated. Similarly, Exograph will ensure that the `performances` are associated with the updated concert. In other words, you don't have to worry about setting the concert id in the nested mutations.

## Deleting data

To delete a single entity by its primary key, Exograph offers the `delete<EntityType>` mutation, which takes the primary key as an argument.

Given this mutation, you can delete a concert as follows:

```graphql
mutation {
  deleteConcert(id: 1) {
    id
    title
    startTime
  }
}
```

Like the [query to delete a single entity](queries.md#primary-key-query) and [update a single entity](mutations.md#updating-a-single-entity), if the entity type has a composite primary key, you must supply all the fields of the primary key as arguments to the mutation.

```graphql
mutation {
  deletePerson(firstName: "John", lastName: "Doe") {
    id
    firstName
    lastName
  }
}
```

To delete multiple entities, Exograph offers the `delete<PluralizedEntityName>` mutation, which takes a `where` argument to filter the entities to be deleted (it is the same `where` argument that is used to filter data in the queries in the [earlier section](queries.md#collection-query)). If you want to delete all concerts hosted in venue 3, you could do so as follows:

```graphql
mutation {
  deleteConcerts(where: { venue: { id: { eq: 3 } } }) {
    id
    title
  }
}
```

Like all mutations, delete mutations return the deleted entity (and you can select the field you want to retrieve as with any query), which can be helpful for the client to update its cache.
