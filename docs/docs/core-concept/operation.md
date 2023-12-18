---
sidebar_position: 4
---

# Queries and Mutations

A query is a read-only operation such as fetching data or performing computations. Using the `query` keyword, you may define queries specific to the module.

```exo
@deno("email.ts")
module EmailModule {
  query status(): Boolean
}
```

In this case, the Deno plugin will associate the query with the same-named function in `email.ts`. Then to execute the query, Exograph will call that function with the arguments supplied and return the function's return value.

Note that not all plugins support explicitly defined queries. For example, the Postgres plugin only supports queries inferred from the types defined in the module.

A mutation is a write operation such as updating some data source. Like queries, mutations are defined using the `mutation` keyword.

```exo
@deno("email.ts")
module EmailModule {
  mutation sendEmail(to: String, subject: String, body: String): Boolean
}
```

:::warning
Although queries and mutations look alike, pay attention to their semantics. Queries should act as read-only operations, whereas mutations can update data. A plugin may use these semantics for optimizations, such as caching results.
:::

Exograph leaves the interpretation of the mutation definition up to the plugin. The Deno plugin interprets the mutation definition as a function to be called when executing the mutation.
