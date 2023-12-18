---
sidebar_position: 20
---

# Securing the APIs

So far, we have implemented the model, but we have an overly lax access control rule: `@access(true)`. This rule allows _anyone_ to query, create, update, and delete a concert or venue. To fix this, let's add a more meaningful access control.

## Specifying Access Control

We want to specify that:

- A query access rule: Admins can access any concert or venue, but non-admins can query only published concerts and venues.
- A mutation access rule: Only admin users can create, update, or delete a concert or venue.

Before specifying the access control, we need to define a [context](/core-concept/context.md), which is a way to capture information from the request. In this case, we want to capture the user's role from the JWT token. To do that, we will use the `@jwt` annotation.

```exo
context AuthContext {
  @jwt role: String
}
```

We could also capture other JWT claims, such as `sub`, which would be helpful to create rules based on the user's id. But for this application, where access control only cares about the user's role, we don't need them.

Now, we can specify precise access control by replacing `access(true)` as follows:

```exo
@postgres
module ConcertData {
  // highlight-next-line
  @access(query = AuthContext.role == "admin" || self.published, mutation = AuthContext.role == "admin")
  type Concert {
    ...
  }

  // highlight-next-line
  @access(query = AuthContext.role == "admin" || self.published, mutation = AuthContext.role == "admin")
  type Venue {
    ...
  }
}
```

This access control rule allows anyone to query a concert or venue if it is published. But it lets only admins query unpublished concerts and venues. It also allows only admins to mutate (create, update, or delete) a concert or venue.

:::tip
If you are following along by performing steps in the earlier section and keep running `exo yolo`, Exograph will automatically pick up these changes. No need to restart the server!
:::

## Trying out the Access Control

Let's try out the access control rules. First, let's try to query all the venues:

### Without authentication

Now, let's try out a mutation to create a new venue.

```graphql
mutation {
  createVenue(data: { name: "Carnegie Hall", published: true }) {
    id
    name
    published
  }
}
```

We get an error:

```json
{
  "errors": [
    {
      "message": "Not authorized"
    }
  ]
}
```

Since we didn't specify a JWT access token along with the request, the incoming user is "anonymous". Since only admins can perform mutations, Exograph correctly rejected this request with a "Not authorized" error. Nice!

### With authentication in the Playground

Let's try to become an admin and try again. While in a typical application, you will have an authentication mechanism to create the JWT token, we will do it through Exograph's playground.

import symmetricAuthPlayground from '../authentication/playground/images/symmetric-auth-playground.mp4';

<video controls width="100%">
  <source src={symmetricAuthPlayground}/>
</video>

In the playground, click on the "Authenticate" button in the middle center of the screen. That will pop up a dialog box. In the dialog box, enter the following:

- For "Secret", enter the secret printed by the `exo yolo` command.
- For "Claims", enter the following:

```json
{
  "role": "admin"
}
```

And click "Sign In". This will create a JWT token and pass it to each request in the `Authorization` header.

Now, let's retry the same mutations. You should see the new venue in response.

Now try querying all the venues:

```graphql
query {
  venues {
    id
    name
    published
    concerts {
      id
      title
      published
    }
  }
}
```

Or all the concerts:

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

And you will see all the venues or concerts--`published` or not--in the response.

Now click the "Authorization" button and then the "Sign Out" button. This will sign out the current user. Let's try the same queries one last time. Here, you will only see the published venues and concerts in the response, which is the desired behavior: non-admins can only see published concerts.

Exograph helps you create GraphQL API with minimal effort and secure them easily and clearly. And its playground makes it easy to explore the API and see how the access control rules work.
