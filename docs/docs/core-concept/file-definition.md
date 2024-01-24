---
sidebar_position: 0
---

# Exograph file definition

Exograph files define the model of the backend services in a file with the `.exo` extension. The Exograph language takes inspiration from TypeScript and the GraphQL schema definition language. However, it is not a subset or superset of either (see [FAQ](/faq.md) for more information).

An Exograph file may include four top-level elements: [contexts](context.md), [modules](module.md), [imports](import.md), and of course, [comments](#comments). Most Exograph modules also include [types](type.md), [queries and mutations](operation.md), and [interceptors](interceptor.md).

## Annotations

Exograph elements can have annotations that precede them. Annotations provide additional information to the plugins that interpret the elements. For example, the `@pk` annotation preceding a field specifies that it is the primary key.

## Comments

Exograph files support line comments using the `//` syntax and block comments using the `/* */` syntax. Comments may appear anywhere in the file. Here are a few examples:

```exo
// User module describes the user model and operations
module UserModule {
  /* User represents both humans and machines */
  type User { //
    @pk id: Int // The purpose of id is to identify a user
    /* Got to have a name! */ name: /* sure this is a comment, too*/ String /* Another block comment. */
    kind: String // The kind of user: human or machine
  }
}
```
