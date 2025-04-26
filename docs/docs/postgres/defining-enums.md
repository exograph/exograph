---
sidebar_position: 2.5
---

# Defining Enums

Exograph supports enums, which are a way to define a set of allowed values for a field. Enums are defined using the `enum` keyword. 

```exo
@postgres
module TaskDatabase {
  enum Priority {
    LOW
    MEDIUM
    HIGH
  }
}
```

Exograph will map the `Priority` enum to the `priority` enumerated type in the database. For naming, Exograph uses the same convention as for types: it converts the enum name to a "snake_case" name.

Once defined, enums can be used in type definitions as if they were scalars.

```exo
type Todo {
  @pk id: Int = autoIncrement()
  title: String
  priority: Priority = MEDIUM
}
```

The `priority` field is now restricted to the values `LOW`, `MEDIUM`, and `HIGH`. If you try to set it to any other value, Exograph will return an error.

You use fields of enum type in queries and mutations just like any other scalar type. For example, the following mutation creates a new todo with a low priority.

```exo
mutation {
  createTodo(title: "Buy groceries", priority: LOW) {
    id
  }
}
```

Whereas the following query will return all todos with a medium priority.

```exo
query {
  todos(where: { priority: { eq: MEDIUM } }) { 
    id
    title
    priority
  }
}
```
