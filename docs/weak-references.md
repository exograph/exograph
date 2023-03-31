# Managing deletion of related entities

## Motivation

So let's build a concert management system for an organization that regularly
hosts music concerts each featuring a few artists.

Users will first start with the following domain model.

```exograph
export model Concert {
  @pk id: Int = autoIncrement()
  title: String
  concertArtists: Set[ConcertArtist]
}

export model Artist {
  @pk id: Int = autoIncrement()
  name: String
  artistConcerts: Set[ConcertArtist]
}

model ConcertArtist {
  @pk id: Int = autoIncrement()
  concert: Concert
  artist: Artist
  role: String // When we support, an enum { main, accompanying }
  rank: Int    // The importance of the artist in this concert (typically displayed in that order)
}
```

The core idea here is to set up a many-to-many relationship between `Concert`
and `Artist` through a join entity `ConcertArtist`. Since `ConcertArtist` is an
artifact of relational database structure, it doesn't make too much sense to
manipulate it directly.

> Changes so far from the current implementation
>
> - The `export` keyword. Only the `export`ed models have top-level API access;
>   others have to be accessed through an exported entity. For example, there
>   will not be `createConcertArtist`, `updateConcertArtists`, etc.
> - The plain array has been replaced with `Set` to better match the unordered
>   nature and the constraint that each member appears only once. This will also
>   help distinguish from `json_array`, which has the proper array semantics, as
>   a primitive type (see issue #6).

The `Concert` model will have associated APIs such as (and their plural versions
as we have in the current implementation)

- `createConcert`:
  ```graphql
  createConcert(
    data: {
      title: "Evening Vocal Concert",
      concertArtists: [
        { artist: {id: 12}, role: "main", rank: 1 }
      ]
    }
  )
  ```
- `deleteConcert`:
  ```graphql
  deleteConcert(id: 100)
  ```
- `updateConcert`:
  ```graphql
  updateConcert(
    id: 100,
    data: {
        title: "Evening Vocal Concert",
        concertArtists: [
          { artist: {id: 12}, role: "main", rank: 1 }
          { artist: {id: 22}, role: "accompanying", rank: 2 }
          { artist: {id: 32}, role: "accompanying", rank: 3 }
        ]
      }
    )
  ```

## Issues

- How can one delete a concert since just deleting it (without any cascade) will
  violate the foreign key constraint on the `concert_artist` table?
- If we say, let's delete the related entries in `ConcertArtist`, would the same
  logic apply if an `Artist` is deleted? What if the user prefers an error in
  that case to force the end-user to deal with concerts that host those artists
  first (otherwise, we can get into artist-less concerts!).
- As it is set up, there is no way to remove an artist once added to a concert:
  `ConcertArtist` apis are not exposed and foreign key constraints will prevent
  deleting them when deleting a `Concert` or an `Artist`.

## Proposal

We look at the whole problem as that of Garbage Collection (GC). Like GC, we
won't delete any objects that have other (strong) references and introduce the
notion of weak references.

> **Semantics**
>
> 1. A weak reference holds onto an object such that it doesn't prevent the
>    object from getting deleted if there is no way to reach to that object
>    through a chain of strong references. Colloquially, weak references may be
>    snatched away by a change to another entity.
> 2. By default, the database (conceptually the root object) holds onto all
>    **exposed** model elements strongly.

### Weak collections

To model the requirement that deleting a `Concert` should delete
`ConcertArtist`s, but deleting an `Artist` should produce an error if that
`Artist` is being referred from a `ConcertArtist` (which implies being
associated with a `Concert`). This can be accomplished by replacing the type of
`artistConcerts` in `Artist` to `WeakSet`:

```exograph
export model Artist {
  @pk id: Int = autoIncrement()
  name: String
  artistConcerts: WeakSet[ConcertArtist]
}
```

Here since `Artist` refers to `artistConcerts` only weakly, the system can
delete `ConcertArtist` for when deleting an `Artist` only if there are no strong
references from anywhere. So deleting an artist performing in a concert will
result in a constraint violation error that the user will have to resolve by
first deleting the concerts referring to that artist. On the other hand,
deleting a concert will snatch that concert from the artist.

### Weak models

To promote the fact that it hosts many top-notch artists, the organization wants
to designate a few appearances as "featured" (and show them prominently on the
home page). We model this requirement by introducing the `FeaturedPerformance`
model.

```exograph
model ConcertArtist {
  ... same as earlier
  featured: FeaturedPerformance?
}

export model FeaturedPerformance {
  @pk id: Int = autoIncrement()
  promoTitle: String
  concertArtist: ConcertArtist
}
```

This change will prevent a `Concert` from getting deleted if that concert is
promoted (there is a strong reference from `FeaturedPerformance` to
`ConcertArtist` and thus to a `Concert`). This is what may be desired, but what
if the expected behavior is to delete any `FeaturedPerformance`s along with the
`Concert`?

We cannot simply mark `concertArtist` in `FeaturedPerformance` as `Weak`, since
that will mean that the `concertArtist` will be set to NULL.

Instead, we want the system to hold `FeaturedPerformance` weakly. We do so by
using the `@weak` annotation as follows:

```exograph
@weak export model FeaturedPerformance {
  ... same as earlier
}
```

### Weak references

Following a concert, the organization publishes a summary of the performance and
lets authenticated users post comments.

```exograph
export model Post {
  ...
  comments: Set[Comment]
}

export model User {
  ...
  comments: Set[Comment]
}

export model Comment {
  ...
  post: Post
  user: User
}
```

Here nothing can be deleted once created: System holds onto all exported objects
strongly.

**Scenario 1**: What if a user wants to delete a comment? We need to allow
comments to be snatched away from the associated `Post` (and from the `User`).
So we make `comments` in both entities as `WeakSet`. Now deleting a comment
removes it from the `Post` and the `User`.

```exograph
export model Post {
  ...
  comments: WeakSet[Comment]
}

export model User {
  ...
  comments: WeakSet[Comment]
}
```

**Scenario 2**: Allow deleting a user account. As modeled above, a user who has
made a comment cannot be deleted. We have two options:

1. Delete comments made by the user. This requires the following change:

```exograph
// Same model as in Scenario 1

@weak export model Comment {
  ...
  post: Post
  user: User
}
```

This change allows comments to be snatched away from the root object.

2. Set the comment's user to NULL (i.e. mark the comment as posted by an
   anonymous user). This requires the following change:

```exograph
// Same model as in Scenario 1

export model Comment {
  ...
  post: Post
  user: Weak[User]
}
```

This allows the user to be snatched away from a comment.

## Implementation notes

While we conceptualize the system as a GC problem, we cannot afford to implement
using the traditional GC techniques. And even if we did, we will have to do the
GC sweep synchronously during the operation itself, otherwise objects will
linger for a duration and will be seen by other calls. Instead, we analyze the
model and statically set constraints to the extent possible and execute any
residue dynamically with the query (similar to how we do it for access control).

1. When possible we produce the schema with an appropriate
   [ON DELETE](https://www.postgresql.org/docs/9.5/ddl-constraints.html) value.
   So given this:

```exograph
export model Artist {
  @pk id: Int = autoIncrement()
  name: String
  artistConcerts: WeakSet[ConcertArtist]
}
```

We produce:

```sql
CREATE TABLE concert_artist (
  artist_id INTEGER REFERENCES artists ON DELETE RESTRICT,
  concert_id INTEGER REFERENCES concerts ON DELETE CASCADE,
  ...
)
```

Now deleting a `ConcertArtist` while deleting a `Concert` will cause that
`Concert` to be allowed to be deleted.

We have to be careful to make sure that the access rules also align well (in
most normal business domains, they will).

### Rules

A. A `Weak[T]` reference is always emitted as `ON DELETE SET NULL`. B. For
non-weak exported models, each strong reference is emitted as
`ON DELETE RESTRICT`.

```exograph
export model Comment {
  ...
  post: Post
  user: Weak[User]
}
```

```sql
CREATE TABLE comments (
  user_id INTEGER REFERENCES users ON DELETE SET NULL,
  post_id INTEGER REFERENCES posts ON DELETE RESTRICT
  ...
)
```

With this arrangement, deleting a comment will set the `user_id` to NULL.

C. For `@weak` and not exported models, emit each strong reference as
`ON DELETE CASCADE`.

```exograph
@weak export model FeaturedPerformance {
  @pk id: Int = autoIncrement()
  promoTitle: String
  concertArtist: ConcertArtist
}
```

```sql
CREATE TABLE featured_performances (
  id INTEGER PRIMARY KEY,
  ...
  concert_artist_id INTEGER REFERENCES concert_artists ON DELETE CASCADE
)
```

With this arrangement, deleting a `ConcertArtist` (by deleting a `Concert`) will
allow removing the corresponding `ConcertArtist` and thus allowing deleting the
concert.

2. Runtime check for access control

A. Posts belong to its author, comments belong to the user. What if the author
wants to delete a post?

A1. Fail the deletion

Keep the original model (and let admin do this manually assume admin has access
to the models involved).

A2. But let comments stay (with a null post)?

```exograph
@access(mutation: AuthContext.user.id == self.user.id)
model Comment {
  post: Weak[Post]
}
```

Here even though the post author doesn't have any mutation rights to the
comment, it can snatch the post from a comment (replacing a `Weak` with a NULL
doesn't count as a mutation)

A3. Delete comments along with the post

```exograph
@access(mutation: AuthContext.user.id == self.user.id, transitiveDelete: true)
model Comment {
  post: Post
}
```

## Caveats

- How to teach users not familiar with the "weak" reference concept?
- Will this be perceived as too complicated?
- How to handle legacy database schemas (whose cascades don't conform to our
  model)? Show as an error? Show as a warning and do runtime checks?
