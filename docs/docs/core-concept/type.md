---
sidebar_position: 3
---

# Types

A type defines a data structure that a plugin can infer to provide queries and mutations. Types can also be used while defining queries and mutations. A type defines a set of fields that may be scalar types or other types. For example, the following type defines a `Todo` type with three fields: `id`, `title`, and `completed`.

```exo
@postgres
module TodoPersistence {
  type Todo {
    id: Int
    title: String
    completed: Boolean
  }
}
```

The plugin, in this case, the Postgres plugin, will interpret the type definition as a database entity and create queries such as `todos` and `todo` to retrieve the entities from the database and mutations such as `createTodo` and `updateTodo` to create and update the entities in the database.

Exograph supports optional semi-colons at the end of each line. Therefore, the above definition is equivalent to the following:

```exo
@postgres
module TodoPersistence {
  type Todo {
    id: Int;
    title: String;
    completed: Boolean;
  }
}
```

Each type may carry annotations, and as you may have guessed by now, the plugin is responsible for interpreting those. For example, the Postgres plugin will interpret the `@table` annotation as the table's name in the database to override the one inferred from the type name.
