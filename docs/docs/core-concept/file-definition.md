---
sidebar_position: 0
slug: /core-concept
---

# Exograph file definition

Exograph files define the model of the backend services in a file with the `.exo` extension. The Exograph language takes inspiration from TypeScript and the GraphQL schema definition language. However, it is not a subset or superset of either (see [FAQ](/faq.md) for more information).

An Exograph file may include four top-level elements: [contexts](context.md), [modules](module.md), [imports](import.md), and of course, [comments](#comments). Most Exograph modules also include [types](type.md), [queries and mutations](operation.md), and [interceptors](interceptor.md).

## Annotations

Exograph elements can have annotations that precede them. Annotations provide additional information to the plugins that interpret the elements. For example, the `@pk` annotation preceding a field specifies that it is the primary key.

## Comments

Exograph files support regular comments and documentation comments.

### Regular Comments

Regular comments use the `//` syntax for line comments and `/* */` syntax for block comments. Comments may appear anywhere in the file. Here are a few examples:

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

### Documentation Comments

Documentation comments are special comments that become part of the GraphQL schema description and are included in introspection results. Exograph supports two types of documentation comments (which follow conventions from [Rust](https://doc.rust-lang.org/book/ch14-02-publishing-to-crates-io.html#the-doc-attribute)):

#### Element Documentation Comments

Element documentation comments use `///` for line comments or `/** */` for block comments and document specific elements like types, fields, methods, and interceptors:

```exo
@postgres
/// Todo database module for managing user tasks
module TodoModule {
  /// Represents a todo item with user ownership
  @access(query=true, mutate=true)
  type Todo {
    /// Unique identifier for the todo
    @pk id: Int
    
    /**
     * The todo's title or description
     * This field is required and cannot be empty
     */
    title: String
    
    /// Whether the todo is completed
    completed: Boolean
  }
}

@deno("todos.ts")
module TodoService {
  /// Computes the effort required to complete a todo
  query computeEffort(id: Int): Int
}
```

#### Global Documentation Comments

Global documentation comments use `//!` for line comments or `/*! */` for block comments and provide top-level documentation for the entire schema:

```exo
//! Multi-user todo application model. Users can only query/mutate their own todos. Admins can query/mutate all todos.

/*!
 * The default user role is "user".
 * The default priority is "medium".
 */

context AuthContext {
  @jwt("sub") id: Int
  @jwt role: String
}
```

Documentation comments are particularly useful when using Exograph with MCP (Model Context Protocol), as they provide LLMs with context about your data model and operations.


