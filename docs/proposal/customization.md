Claytip offers automatically generated API based on the model and thus effectively based on the database schema. This is wonderful to start with since it let's the UI to have type-safe, secure access to the data without thinking too much about the usage patterns (which is unlikely to be clear in the early days of any project). But that also has a few disadvantages:

- Any changes to a model field (add/remove/rename) will have an immediate impact on the filtering and ordering types as well as return types. All existing clients will then be rendered incompatible.
- The API is very much has SQL-through-GraphQL feel. This is potentially problematic as the project grows. Specifically, there is no guidance for the clients to know the expected usage patterns and as a result, different clients will end up using different styles and that makes managing the backend for efficiency difficult.

This proposal outlines a few features that will avoid these issues.

## Protection against API drift with aliases

Suppose a model started out as:

```clay
model Concert {
  id: ...
  headline: String
}
```

This will lead to API such as:

```graphql
concerts(where: {headline: {eq: "vocal"}}, orderBy: {headline: ASC}) {
  id
  headline
}
```

If a developer decided to rename `headlines` to `title`, all existing clients will be instantly broken. So we allow the `@aliases` annotation to ease the process.

```
model Concert {
  id
  title: String @aliases("headline")
}
```

The `@aliases` annotation accepts an array of strings. Following the GraphQL convention, we will automatically lift a single string into a single-element array.

With this change:

- All filtering, ordering operation as well as selection will be available under the new name as well as names provided in `@aliases`.
- All aliased names will be marked as deprecated (for introspection purpose).

## Explicit deprecation

If a developer plans to remove a field in the future, that field can be marked as `@deprecated`. There will be no impact on any filtering, ordering, or selection, but in each case the corresponding entries will be deprecated through introspection.

## Purpose-specific primitive types

Consider the following model

```clay
model Venue {
  id: ...
  zipCode: Int
}
```

This will offer filtering operators such as `lt`, `gt` in addition to `eq`. The comparator operator makes no sense in most applications. The API will also allow ordering by zipCode. It too makes little sense for most applications (but may be useful in some cases to allow spreadsheet-style sorting--basically, depends on the intended usage).

The proposal allows user-defined primitives:

```clay
primitive ZipCode: Int @range(min: 1, max: 99999) {
  operators: [eq]
  ordering: []

}
```

Here the `ZipCode` is a primitive type derived from `Int`. It specifies that a zip code must be in the 1-99999 range (we allow all annotations that you may specify for the underlying primitive type here).

Other user-specified primitive types examples:

- `PK` to avoid repeated `Int @dbtype("BIGINT") @pk @autoincrement` for every single model type
- `Description` to removed `lt`, `gt` etc on a long text
- `HashId` to remove all operators except `eq` and remove all ordering

TODO: We should be able to allow custom operators in the same way.

## Exposing business-specific queries

```graphql
query nextConcert: Concert = concerts(where: { date: { gt: $TODAY }}, orderBy: { date: ASC }, limit: 1}) @unique
```

Here the `@unique` annotation notifies the type system that one single (possibly `null`) result is expected of this query and the runtime will enforce that there is only one item returned in such query.

The issue here is resolving the `$TODAY` variable. For that we allow services to define custom values.

```clay
@extrnal("concert_support.ts")
service ConcertSupport {
  val TODAY = sql"TODAY()"

  fn RANDOM(): Int
}
```

Here `val` implies that the function needs to be evaluated only once. The `RANDOM` function, on the other hand, will evaluate every time is is used (and in the same way as any service function).

TODO: Specify the `use` keyword to bring in a service into the current scope. For example, instead of specifying `$ConcertSupport.TODAY`, developer could add `use ConcertSupport.*;` and use `$TODAY`.

```graphql
query concertAfterDate(date: DateTime): Concert = concerts(where: { date: { gt: $date }}, orderBy: { date: ASC }, limit: 1}) @unique
```

Aside: If you have this query, the `nextConcert` could be rewritten more simply as:

```graphql
query nextConcert: Concert = concertAfterDate(date: $TODAY)
```

```clay
query concertAfter(concert_id: Int, @inject clay: Clay): Concert = external
```

```ts
concertAfter(concert_id: Int, @inject clay: Clay): Concert {
  let endTime = await clay.execute(concert(id: concert_id) {
    endTime
  });
  await clay.execute("concertAfterDate{ date: endTime}");
}
```

TODO: We should be able to provide a more direct support for simple queries such as this (one queries output flows into the next queries input).

```

```
