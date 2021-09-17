Claytip offers automatically generated API based on the model and thus effectively based on database schema. This is wonderful to start with since it let's the UI to have type-safe, secure access to the data without thinking too much about the usage patterns (which is unlikely to be clear in the early days of any project).

- Any changes to a model field (add/remove/rename) will have an immediate impact on the filtering and ordering types.
- The API is very much has SQL-through-GraphQL feel. This is potentially problematic as the project grows.

## Protection against API drift with aliases

Suppose a model started out as:

```clay
model Concert {
  id: ...
  headline: String]
}
```

This will lead to API such as:

```graphql
concerts(where: {headline: {eq: "vocal"}}, orderBy: {headline: ASC}) {
  id
  headline
}
```

If a developer decided to rename `headlines` to `title`, all existing clients will be instantly broken. So we allow the @aliases annotation to ease the process.

```
model Concert {
  id
  title: String @aliases("headline", )
}
```

With this change:

- All filtering, ordering operation as well as selection will be available under the new name as well as names provided in @aliases.
- All aliased names will be marked as deprecated (for introspection purpose).

## Explicit deprecation

If a developer plans to remove a field in the future, that field can be marked as @deprecated. There will be no impact on any filtering, ordering, or selection, but in each case the corresponding entries will be deprecated through introspection.

## Purpose-specific primitive types

Consider the following model

```clay
model Venue {
  id: ...
  zipCode: Int
}
```

This will offer filtering operators such as `lt`, `gt` in addition to `eq`. The comparator operator makes no sense in most applications. The API will also allow ordering by zipCode. It too may not make sense for most applications.

The proposal is to allow user-defined primitives:

```clay
primitive model Zipcode: Int {
  operators: [eq]
  ordering: []
}
```

Other user-specified primitive types examples:

- `Description` to removed `lt`, `gt` etc on a long text
- `HashId` to remove all operators except `eq` and remove all ordering

## Exposing business-specific queries

query nextConcert: Concert = concerts { where: { date: { gt: $TODAY }}, orderBy: { date: ASC }, limit: 1}} @unique

Here the @unique annotation notifies the type system that one single (possibly null) result is expected of this query and the runtime will enforce that there is only one item returned in such query.

The issue here is the $TODAY variable????

```
service ConcertSupport {
  val TODAY = sql"TODAY()"
}
```

Here `val` implies that the function needs to be evaluated only once.

```
query concertAfterDate(date: DateTime): Concert = concerts { where: { date: { gt: $date }}, orderBy: { date: ASC }, limit: 1}} @unique
```

Aside: If you have this query, the `nextConcert` could be rewritten more simply as:

```
query nextConcert: Concert = concertAfterDate { date: $TODAY }
```

````clay
query concertAfter(concert_id: Int, @inject clay: Clay): Concert = external


```ts
concertAfter(concert_id: Int, @inject clay: Clay): Concert {
  let endTime = await clay.execute(concert(id: concert_id) {
    endTime
  });
  await clay.execute("concertAfterDate{ date: endTime}");
}
````

Note: We should be able to provide a more direct support for simple queries such as this (one queries output flows into the next queries input).
