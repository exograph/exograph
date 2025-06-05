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

The plugin (in this case, the Postgres plugin) will interpret the type definition as a database entity and create queries such as `todos` and `todo` to retrieve the entities from the database, as well as mutations such as `createTodo` and `updateTodo` to create and update the entities in the database.

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

## Fragments

Fragments allow you to specify a group of fields that are used together. For example, you may have types that have a `name` field and an `email` field. You may want to create a fragment that contains both of these fields.

```exo
@postgres
module UserPersistence {
  fragment UserInfoFragment {
    @pk id: Int = autoIncrement()
    name: String
    email: String
  }

  type Employee {
    ...UserInfoFragment
    manager: Manager
  }

  type Manager {
    ...UserInfoFragment
    employees: Set<Employee>
  }
}
```

Fragments are often used to reflect the structure of the database and can be spliced into other types where you may specify access control rules. See [exo schema import](../cli-reference/development/schema/import#creating-fragments-from-the-database) for an automatic way to create fragments from an existing database.
