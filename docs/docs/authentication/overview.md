---
title: Overview
sidebar_position: 10
---

In a typical Exograph application, the model declares a context to capture the user's identity and uses it to specify access control rules, etc. Earlier, when discussing the [context](/core-concept/context.md#jwt-token) concept, we briefly looked at the `@jwt` annotation. Let's take a closer look at this annotation and how to configure JWT authentication.

This section will explore how to set up symmetric and OpenID authentication. We will also explore how to test your authentication logic in the GraphQL Playground.

## The @jwt annotation

Consider the following claims encoded as a JWT token and passed in the `Authorization` header in the form `Bearer <token>`:

```json
{
  "sub": "1234567890",
  "name": "Jordan Taylor",
  "role": "admin",
  "email": "jordan.taylor@example.com"
}
```

In the Exograph definition, you can capture any of these claims using the `@jwt` annotation. For example, you may want to know the user's id (typically available as the `sub` field in a JWT token) and use it to implement access control rules such as "a user can only access only their todos". Similarly, you may want to capture the `role` to implement rules such as "admin users can access any todos". The following context definition will do the job:

```exo
context AuthContext {
  @jwt sub: string
  @jwt role: string
}
```

By default, Exograph assumes that the claim name is the same as the context field name. Thus, the `sub` field is assumed to be the `sub` claim, and the `role` field is assumed to be the `role` claim. You can explicitly specify the claim field name by providing it as the argument to the `@jwt` annotation. For example, since the `sub` field is (typically) the user's id, we may want to rename it to `id` as follows:

```exo
context AuthContext {
  @jwt("sub") id: string
  @jwt role: string
}
```

Once we have the context, we can use it in access control rules, as default values for fields, and as injected arguments to queries, mutations, and interceptors defined in Deno modules.

## Using in access control rules

We explored access control rules in the access control for [Postgres](/postgres/access-control.md) and for [Deno](/deno/access-control.md) section. We will explore it from the `@jwt` annotation's perspective here.

Assume you have a `Todo` type with a `userId` field and the `AuthContext` defined earlier.

```exo
@postgres
module TodoDatabase {
  type Todo {
    @pk id: Int = autoIncrement()
    title: String
    completed: Boolean
    userId: String
  }
}
```

To implement the "a user can only access only their todos" rule as follows, you can attach the following access control rule to the `Todo` type.

```exo
@postgres
module TodoDatabase {
  // highlight-next-line
  @access(self.userId == AuthContext.id)
  type Todo {
    @pk id: Int = autoIncrement()
    title: String
    completed: Boolean
    userId: String
  }
}
```

This rule specifies that to query or mutate a `Todo`, its `userId` field must be the same as the `AuthContext`'s `id` field. You will get an error if you try to query or mutate a `Todo` for another user.

If later you want to implement the "admin users can access any todos" rule, you can do so by adding `|| AuthContext.role == "admin"` to the access control rule:

```exo
@postgres
module TodoDatabase {
  // highlight-next-line
  @access(self.userId == AuthContext.id || AuthContext.role == "admin")
  type Todo {
    @pk id: Int = autoIncrement()
    title: String
    completed: Boolean
    userId: String
  }
}
```

With this rule, if the user is an admin, the expression will evaluate to true, and the user can access any `Todo`.

## Using in default values

With the earlier `Todo` definition, if you wanted to create a new `Todo`, you would need to specify the `userId` field.

```graphql
mutation {
  createTodo(
    data: { title: "Buy milk", completed: false, userId: "1234567890" }
  ) {
    id
    title
    completed
    userId
  }
}
```

This is not ideal since the user's id is already available in the `AuthContext`. We can use the `AuthContext` to set the `userId` field to the user's id as follows:

```exo
@postgres
module TodoDatabase {
  @access(self.userId == AuthContext.id || AuthContext.role == "admin")
  type Todo {
    @pk id: Int = autoIncrement()
    title: String
    completed: Boolean
    // highlight-next-line
    userId: String = AuthContext.id
  }
}
```

With the default value specified, you can omit the `userId` field when creating a new `Todo`.

```graphql
mutation {
  createTodo(data: { title: "Buy milk", completed: false }) {
    id
    title
    completed
    userId
  }
}
```

However, an admin user can still create a `Todo` for another user by explicitly specifying the `userId` field.

## Using in Deno modules

Besides access control rules, you may want to capture the user's identity as an argument to a Deno query, mutation, or interceptor. For example, you may want to log who performed a mutation. You can add the `AuthContext` as an argument to the interceptor.

```exo
@deno("log-mutations.js")
module LogMutations {
  interceptor logMutations(operation: Operation, context: AuthContext)
}
```

```typescript
export function logMutations(operation: Operation, context: AuthContext) {
  const userId = context.id;
  const operationName = operation.name();
  const argsString = JSON.stringify(operation.args);

  console.log(
    `User ${userId} performed ${operationName} with arguments ${argsString}`
  );
}
```
