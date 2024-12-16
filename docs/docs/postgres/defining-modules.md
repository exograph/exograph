---
sidebar_position: 1
---

# Defining Modules

You define a Postgres module using the `@postgres` annotation. The module may contain type definitions, which are mapped to tables in the database.

```exo
@postgres
module TodoDatabase {
  // Types
}
```

The `@postgres` annotation takes two optional parameters:

- `schema`: The default schema for all tables in the module. See [specifying a schema](customizing-types.md#specifying-a-schema) for more details.
- `managed`: The default managed state for all tables in the module. See [unmanaged views](customizing-types.md#unmanaged-views) for more details.

For example, the following module will associated the `Product` type with the `products` table in the `commerce` schema.

```exo
@postgres(schema="commerce")
module Commerce {
  @access(true)
  type Product {
    ...
  }
}
```

While the following definition will mark all the types in the module as unmanaged.

```exo
@postgres(managed=false)
module CommerceViews {
  @access(true)
  type ProductProfit {
    ...
  }
}
```

In the current version, the module name is for organizational purposes only. In future, Exograph will allow database configuration through this annotation and use the name of the module as a namespace for the types defined in it.

In the [next section](defining-types.md), we will look at defining types in a Postgres module.
