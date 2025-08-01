---
sidebar_position: 10
title: Developing the model
---

# The Application

Let's build an Exograph app for a concert management service. We will focus on the following application requirements:

- A concert has a title, may be published, and is hosted in a venue.
- A venue has a name, may be published, and hosts many concerts.
- Users need to be notified of upcoming concerts or any unplanned changes.
- Each query in the system should be monitored for the time it takes to execute.

We will start by building the model using the "yolo" mode to let us focus on the core concepts. Then, we will introduce the development mode to give a taste of what it is like to develop with Exograph. Finally, we will deploy the app to the cloud using traditional and serverless deployments.

## Creating the Model

Let's start by expressing the concept of `Concert`, `Venue, and their relationship.

> Make sure you have followed the steps in the [installation instructions](/getting-started/local.md).

Create a new Exograph project using the `exo new` command.

```shell-session
# shell-command-next-line
exo new concerts-app
# shell-command-next-line
cd concerts-app
```

Replace the content of `src/index.exo` with the following code to model a concert.

```exo
@postgres
module ConcertData {
  type Concert {
    @pk id: Int = autoIncrement()
    title: String
    published: Boolean
    venue: Venue
  }
}
```

In the above code, we created the `Concert` type with four fields:

- `id`: The primary key (due to `@pk`) and is automatically generated by the database (due to the `= autoIncrement()` part).
- `title`: A string field
- `published`: A boolean field.
- `venue`: Refers to the `Venue` type, creating a relationship between the two types.

Now start the server using `exo yolo`, which serves as a scratch pad that creates an _ephemeral_ database, watches the current directory for changes, and launches the server if the model is error-free. As the model changes, it will apply the migration to the database.

> You could alternatively use `exo dev` to start the server. This will require you to create a database manually.

```
$ exo yolo
```

You will see an error in the console since we have not yet created a `Venue` model. Exograph is a type-safe language, so it will not allow you to include a field of an undefined type.

```
error[C000]: Reference to unknown type: Venue
 --> src/index.exo:5:10
  |
5 |   venue: Venue
  |          ^^^^^ unknown type
```

Let's fix that by adding the `Venue` model.

```exo
@postgres
module ConcertData {
  ...

  type Venue {
    @pk id: Int = autoIncrement()
    name: String
    published: Boolean
    concerts: Set<Concert>?
  }
}
```

Here, we have added a new model called `Venue`. The `id` field is the primary key automatically generated by the database. The `name` field is a string field and the `published` field is a boolean field. It also includes a field of the `Set<Concert>?` type designating that a venue can host multiple concerts.

With `exo yolo` watching for any changes, you will see that the errors are gone and the server is running.

```
Change detected, rebuilding and restarting...
Started server on localhost:9876 in 3.75 ms
```

Visit [http://localhost:9876/graphql](http://localhost:9876/graphql) to see the GraphiQL interface. Go ahead and try the following query to get all the concerts along with the venue it is hosted in:

```graphql
query {
  concerts {
    id
    title
    published
    venue {
      name
      published
    }
  }
}
```

Since we have not added any concerts, we expect it to return an empty array. Instead, we get the following error:

```
{
  "errors": [
    {
      "message": "Not authorized"
    }
  ]
}
```

Each type carries an implicit `@access(false)` annotation by default. This access control rule prevents anyone from querying or mutating entities of that type. Let's fix that by adding the following annotation to the `Concert` and `Venue` types.

```exo
@postgres
module ConcertData {
  // highlight-next-line
  @access(true)
  type Concert {
    ...
  }

  // highlight-next-line
  @access(true)
  type Venue {
    ...
  }
}
```

:::warning
In normal development, you won't just attach `@access(true)` to all your types. Instead, you will use the [access control](/postgres/access-control.md) feature to specify it in a business-specific way. We will see how we will do that in a bit.
:::

Try the same query; you will get an empty array as expected.

```json
{
  "data": {
    "concerts": []
  }
}
```

Let's add a few venues and concerts. First, let's create a venue called `The Great Hall`.

```graphql
mutation {
  createVenue(data: { name: "The Great Hall", published: true }) {
    id
    name
    published
  }
}
```

And another venue called `Zellerbach Hall`.

```graphql
mutation {
  createVenue(data: { name: "Zellerbach Hall", published: true }) {
    id
    name
    published
  }
}
```

If you'd like, try out the following query to get all venues:

```graphql
query {
  venues {
    id
    name
    published
  }
}
```

You should see two venues in response.

Next, let's add a couple of concerts.

```graphql
mutation {
  createConcert(
    data: {
      title: "An evening vocal concert"
      published: true
      venue: { id: 1 }
    }
  ) {
    id
  }
}
```

You should see a new concert in response.

Similarly, let's add a second concert, but this time, keep it unpublished (`published: false`).

```graphql
mutation {
  createConcert(
    data: {
      title: "A morning violin concert"
      published: false
      venue: { id: 2 }
    }
  ) {
    id
  }
}
```

Now, we can try out the first query to get all concerts along with their venues.

```graphql
query {
  concerts {
    id
    title
    published
    venue {
      id
      name
      published
    }
  }
}
```

You should see both concerts in the response.

```json
{
  "data": {
    "concerts": [
      {
        "id": 1,
        "title": "An evening vocal concert",
        "published": true,
        "venue": {
          "id": 1,
          "name": "The Great Hall",
          "published": true
        }
      },
      {
        "id": 2,
        "title": "A morning violin concert",
        "published": false,
        "venue": {
          "id": 2,
          "name": "Zellerbach Hall",
          "published": true
        }
      }
    ]
  }
}
```

You can play with various queries to see how Exograph handles them. For example, you may get a particular concert by id:

```graphql
query {
  concert(id: 1) {
    id
    title
    published
    venue {
      id
      name
      published
    }
  }
}
```

Or you may get all concerts hosted in "The Great Hall":

```graphql
query {
  concerts(where: { venue: { name: { eq: "The Great Hall" } } }) {
    id
    title
    published
    venue {
      name
      published
    }
  }
}
```

So far, we can create and query concerts and venues (you can also update or delete them). But with the current unrefined access control, anyone could create a new concert or venue. That is certainly not what we want in a real-world application. So let's add proper [access control](access-control.md).
