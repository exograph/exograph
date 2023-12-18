---
sidebar_position: 5
---

# Access Control

Queries and mutations in Deno modules may be protected using access control rules. You can specify access control rules using the `access` annotation. It takes a boolean expression that the Exograph runtime evaluates to determine if the query or mutation is accessible. An access expression may refer to any of the `context` objects. For example, to allow access to a query or mutation only to `ADMIN` users, you can use the following annotation:

```exo
context AuthContext {
  role: String
}

module UserModule {
  @access(AuthContext.role == "ADMIN" || AuthContext.role == "SUPER_ADMIN")
  type User {
    id: Int
    name: String
  }

  @access(AuthContext.role == "SUPER_ADMIN")
  query getUser(id: Int): User
}
```

We have defined `@access` for both the `User` type and the `getUser` query. The effective access control rule for the `getUser` query is a logical `and` of the access control rules for the `User` type and the `getUser` query. This allows for protecting types and queries/mutations independently. In this example, the access control rule for the `User` type ensures that no matter how each query or mutation is defined, only "ADMIN" or "SUPER_ADMIN" users can access the `User` type.

:::note
By default, following the secure-by-default principle, Exograph marks all types, queries, and mutations as inaccessible (equivalent to specifying the `@access(false)` annotation)
:::

You can combine expressions to form a more complex expression. For example, to allow access to a query or mutation only to `ADMIN` users unless the `DEVELOPMENT` environment is set to true, you can use the following annotation:

```exo
context AuthContext {
  role: String
}

context EnvContext {
  development: Boolean @env("DEVELOPMENT")
}

module UserModule {

  ...

  @access(AuthContext.role == "ADMIN" || EnvContext.development)
  query getUser(id: Int): User
}
```

If you want to expose a query or mutation to all users, you can use the `true` literal expression:

```exo
module MathModule {

  ...

  @access(true)
  query add(a: Int, b: Int): Int
}
```
